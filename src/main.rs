#[macro_use]
extern crate lazy_static;
extern crate pretty_env_logger;

mod controller;
mod date;
mod db;
mod err;
mod generic_trait;
mod parsers;
mod tg;
mod tz;

use async_once::AsyncOnce;
use async_std::task;
use chrono::Utc;
use cron_parser::parse as parse_cron;
use entity::cron_reminder;
use generic_trait::GenericReminder;
use migration::sea_orm::{ActiveValue::NotSet, IntoActiveModel};
use std::time::Duration;
use teloxide::{
    prelude::*,
    types::{Chat, User},
};

/// Iterate every second all reminders and send notifications if time's come
async fn reminders_pooling(db: &db::Database, bot: Bot) {
    loop {
        let reminders = db.get_active_reminders().await.unwrap();
        for reminder in reminders {
            if let Some(user_id) = reminder.user_id.map(|x| UserId(x as u64)) {
                if let Ok(Some(user_timezone)) =
                    tz::get_user_timezone(db, user_id).await
                {
                    match tg::send_message(
                        &reminder
                            .clone()
                            .into_active_model()
                            .to_string(user_timezone),
                        &bot,
                        ChatId(reminder.chat_id),
                    )
                    .await
                    {
                        Ok(_) => db
                            .mark_reminder_as_sent(reminder.id)
                            .await
                            .unwrap_or_else(|err| {
                                dbg!(err);
                            }),
                        Err(err) => {
                            dbg!(err);
                        }
                    }
                }
            }
        }
        let cron_reminders = db.get_active_cron_reminders().await.unwrap();
        for cron_reminder in cron_reminders {
            if let Some(user_id) =
                cron_reminder.user_id.map(|x| UserId(x as u64))
            {
                if let Ok(Some(user_timezone)) =
                    tz::get_user_timezone(db, user_id).await
                {
                    let new_time = parse_cron(
                        &cron_reminder.cron_expr,
                        &Utc::now().with_timezone(&user_timezone),
                    )
                    .map(|user_time| user_time.with_timezone(&Utc));
                    let new_cron_reminder = match new_time {
                        Ok(new_time) => Some(cron_reminder::Model {
                            time: new_time.naive_utc(),
                            ..cron_reminder.clone()
                        }),
                        Err(err) => {
                            dbg!(err);
                            None
                        }
                    };
                    let message = match &new_cron_reminder {
                        Some(next_reminder) => format!(
                            "{}\n\nNext time → {}",
                            cron_reminder
                                .clone()
                                .into_active_model()
                                .to_string(user_timezone),
                            next_reminder
                                .clone()
                                .into_active_model()
                                .serialize_time(user_timezone)
                        ),
                        None => cron_reminder
                            .clone()
                            .into_active_model()
                            .to_string(user_timezone),
                    };
                    match tg::send_message(
                        &message,
                        &bot,
                        ChatId(cron_reminder.chat_id),
                    )
                    .await
                    {
                        Ok(_) => {
                            db.mark_cron_reminder_as_sent(cron_reminder.id)
                                .await
                                .unwrap_or_else(|err| {
                                    dbg!(err);
                                });
                            if let Some(new_cron_reminder) = new_cron_reminder {
                                let mut new_cron_reminder: cron_reminder::ActiveModel = new_cron_reminder.into();
                                new_cron_reminder.id = NotSet;
                                db.insert_cron_reminder(new_cron_reminder)
                                    .await
                                    .unwrap_or_else(|err| {
                                        dbg!(err);
                                    });
                            }
                        }
                        Err(err) => {
                            dbg!(err);
                        }
                    }
                }
            }
        }
        task::sleep(Duration::from_secs(1)).await;
    }
}

fn set_token(token: &str) {
    std::env::set_var("TELOXIDE_TOKEN", token);
}

// Create static async_once database pool
lazy_static! {
    static ref DATABASE: AsyncOnce<db::Database> =
        AsyncOnce::new(async { db::Database::new().await.unwrap() });
}

async fn run() {
    pretty_env_logger::init();
    log::info!("Starting remindee bot!");

    // Create necessary database tables if they do not exist
    DATABASE.get().await.apply_migrations().await.unwrap();

    // Set token from an environment variable
    let token = std::env::var("BOT_TOKEN")
        .expect("Environment variable BOT_TOKEN is not set");
    set_token(&token);
    let bot = Bot::from_env();

    // Run a database polling loop checking pending reminders asynchronously
    tokio::spawn(reminders_pooling(DATABASE.get().await, bot.clone()));

    let handler = dptree::entry()
        .branch(Update::filter_message().endpoint(message_handler))
        .branch(Update::filter_callback_query().endpoint(callback_handler));

    Dispatcher::builder(bot, handler)
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;
}

impl<'a> controller::TgController<'a> {
    pub async fn new(
        bot: &'a Bot,
        chat: &'a Chat,
        user: &'a User,
        msg: &'a Message,
    ) -> Result<controller::TgController<'a>, err::Error> {
        let is_group = !msg.chat.is_private();
        Ok(Self {
            db: DATABASE.get().await,
            bot,
            chat_id: chat.id,
            user_id: user.id,
            msg_id: msg.id,
            is_group,
        })
    }
}

async fn message_handler(msg: Message, bot: Bot) -> Result<(), err::Error> {
    let mut ctl = controller::TgController::new(
        &bot,
        &msg.chat,
        msg.from()
            .ok_or_else(|| err::Error::UserNotFound(msg.clone()))?,
        &msg,
    )
    .await?;
    if let Some(text) = msg.text() {
        match text {
            "/start" => ctl.start().await,
            "list" | "/list" => ctl.list().await,
            "tz" | "/tz" | "timezone" | "/timezone" => {
                ctl.choose_timezone().await
            }
            "mytz" | "/mytz" | "mytimezone" | "/mytimezone" => {
                ctl.get_timezone().await
            }
            "del" | "/del" | "delete" | "/delete" => ctl.start_delete().await,
            "edit" | "/edit" => ctl.start_edit().await,
            "help" | "/help" => ctl.list_commands().await,
            text => match (
                ctl.get_edit_reminder().await,
                ctl.get_edit_cron_reminder().await,
            ) {
                (Ok(Some(edit_reminder)), _) => {
                    ctl.replace_reminder(text, edit_reminder.id).await
                }
                (_, Ok(Some(edit_cron_reminder))) => {
                    ctl.replace_cron_reminder(text, edit_cron_reminder.id).await
                }
                _ => ctl.set_reminder(text, false).await.map(|_| ()),
            },
        }
    } else if !ctl.is_group {
        ctl.incorrect_request().await
    } else {
        Ok(())
    }
    .map_err(From::from)
}

async fn callback_handler(
    cb_query: CallbackQuery,
    bot: Bot,
) -> Result<(), err::Error> {
    if let Some(cb_data) = &cb_query.data {
        if let Some(msg) = &cb_query.message {
            let mut ctl = controller::TgController::new(
                &bot,
                &msg.chat,
                &cb_query.from,
                msg,
            )
            .await?;
            if let Some(page_num) = cb_data
                .strip_prefix("seltz::page::")
                .and_then(|x| x.parse::<usize>().ok())
            {
                ctl.select_timezone_set_page(page_num)
                    .await
                    .map_err(From::from)
            } else if let Some(tz_name) = cb_data.strip_prefix("seltz::tz::") {
                ctl.set_timezone(tz_name).await.map_err(From::from)
            } else if let Some(page_num) = cb_data
                .strip_prefix("delrem::page::")
                .and_then(|x| x.parse::<usize>().ok())
            {
                ctl.delete_reminder_set_page(page_num)
                    .await
                    .map_err(From::from)
            } else if let Some(rem_id) = cb_data
                .strip_prefix("delrem::rem_alt::")
                .and_then(|x| x.parse::<i64>().ok())
            {
                ctl.delete_reminder(rem_id).await.map_err(From::from)
            } else if let Some(cron_rem_id) = cb_data
                .strip_prefix("delrem::cron_rem_alt::")
                .and_then(|x| x.parse::<i64>().ok())
            {
                ctl.delete_cron_reminder(cron_rem_id)
                    .await
                    .map_err(From::from)
            } else if let Some(page_num) = cb_data
                .strip_prefix("editrem::page::")
                .and_then(|x| x.parse::<usize>().ok())
            {
                ctl.edit_reminder_set_page(page_num)
                    .await
                    .map_err(From::from)
            } else if let Some(rem_id) = cb_data
                .strip_prefix("editrem::rem_alt::")
                .and_then(|x| x.parse::<i64>().ok())
            {
                ctl.edit_reminder(rem_id).await.map_err(From::from)
            } else if let Some(cron_rem_id) = cb_data
                .strip_prefix("editrem::cron_rem_alt::")
                .and_then(|x| x.parse::<i64>().ok())
            {
                ctl.edit_cron_reminder(cron_rem_id)
                    .await
                    .map_err(From::from)
            } else {
                Err(err::Error::UnmatchedQuery(cb_query))
            }
        } else {
            Err(err::Error::NoQueryMessage(cb_query))
        }
    } else {
        Err(err::Error::NoQueryData(cb_query))
    }
}

#[tokio::main]
async fn main() {
    run().await;
}
