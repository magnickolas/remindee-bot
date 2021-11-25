#[macro_use]
extern crate lazy_static;

mod controller;
mod date;
mod db;
mod err;
mod tg;
mod tz;

use async_std::task;
use chrono::Utc;
use cron_parser::parse as parse_cron;
use std::time::Duration;
use teloxide::dispatching::update_listeners::{
    polling_default, AsUpdateStream,
};
use teloxide::prelude::*;
use teloxide::types::UpdateKind;
use tg::GenericReminder;

/// Iterate every second all reminders and send notifications if time's come
async fn reminders_pooling(mut database: db::Database, bot: Bot) {
    loop {
        let reminders = database.get_active_reminders().await.unwrap();
        for reminder in reminders {
            if let Ok(Some(user_timezone)) =
                database.get_user_timezone(reminder.user_id).await
            {
                match tg::send_message(
                    &reminder.to_string(user_timezone),
                    &bot,
                    reminder.user_id,
                )
                .await
                {
                    Ok(_) => database
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
        let cron_reminders =
            database.get_active_cron_reminders().await.unwrap();
        for cron_reminder in cron_reminders {
            if let Ok(Some(user_timezone)) =
                database.get_user_timezone(cron_reminder.user_id).await
            {
                match tg::send_message(
                    &cron_reminder.to_string(user_timezone),
                    &bot,
                    cron_reminder.user_id,
                )
                .await
                {
                    Ok(_) => {
                        database
                            .mark_cron_reminder_as_sent(cron_reminder.id)
                            .await
                            .unwrap_or_else(|err| {
                                dbg!(err);
                            });
                        let new_time = parse_cron(
                            &cron_reminder.cron_expr,
                            &Utc::now().with_timezone(&user_timezone),
                        )
                        .map(|user_time| user_time.with_timezone(&Utc));
                        match new_time {
                            Ok(new_time) => {
                                let new_cron_reminder =
                                    db::CronReminderStruct {
                                        time: new_time.naive_utc(),
                                        ..cron_reminder
                                    };
                                database
                                    .insert_cron_reminder(&new_cron_reminder)
                                    .await
                                    .unwrap_or_else(|err| {
                                        dbg!(err);
                                    });
                            }
                            Err(err) => {
                                dbg!(err);
                            }
                        }
                    }
                    Err(err) => {
                        dbg!(err);
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

async fn run() {
    teloxide::enable_logging!();
    log::info!("Starting remindee bot!");

    let database = db::Database::new().await.unwrap();

    // Create necessary database tables if they do not exist
    database.create_reminder_table().await.unwrap();
    database.create_cron_reminder_table().await.unwrap();
    database.create_user_timezone_table().await.unwrap();

    // Set token from an environment variable
    let token = std::env::var("BOT_TOKEN")
        .expect("Environment variable BOT_TOKEN is not set");
    set_token(&token);
    let bot = Bot::from_env();
    let updater = polling_default(bot.clone());

    // Run a database polling loop checking pending reminders asynchronously
    tokio::spawn(reminders_pooling(database.clone(), bot.clone()));

    // Run a telegram polling loop waiting messages from users and responding to them
    updater
        .await
        .as_stream()
        .fold(database, |mut database, update| async {
            let mut tg_bot = controller::TgBot {
                database: &mut database,
                bot: &bot,
            };
            match update {
                Ok(update) => match update.kind {
                    UpdateKind::Message(msg) => {
                        let user_id = msg.chat_id();
                        if let Some(text) = msg.text() {
                            match text {
                                "/start" => tg_bot.start(user_id).await,
                                "list" | "/list" => tg_bot.list(user_id).await,
                                "tz" | "/tz" | "timezone" | "/timezone" => {
                                    tg_bot.choose_timezone(user_id).await
                                }
                                "mytz" | "/mytz" | "mytimezone"
                                | "/mytimezone" => {
                                    tg_bot.get_timezone(user_id).await
                                }
                                "del" | "/del" | "delete" | "/delete" => {
                                    tg_bot.start_delete(user_id).await
                                }
                                "edit" | "/edit" => {
                                    tg_bot.start_edit(user_id).await
                                }
                                "help" | "/help" => {
                                    tg_bot.list_commands(user_id).await
                                }
                                text => match (
                                    tg_bot.get_edit_reminder(user_id).await,
                                    tg_bot
                                        .get_edit_cron_reminder(user_id)
                                        .await,
                                ) {
                                    (Ok(Some(edit_reminder)), _) => {
                                        tg_bot
                                            .replace_reminder(
                                                text,
                                                user_id,
                                                edit_reminder.id,
                                                msg.from().map(|user| user.id),
                                            )
                                            .await
                                    }
                                    (_, Ok(Some(edit_cron_reminder))) => {
                                        tg_bot
                                            .replace_cron_reminder(
                                                text,
                                                user_id,
                                                edit_cron_reminder.id,
                                                msg.from().map(|user| user.id),
                                            )
                                            .await
                                    }
                                    _ => tg_bot
                                        .set_reminder(
                                            text,
                                            user_id,
                                            msg.from().map(|user| user.id),
                                            false,
                                        )
                                        .await
                                        .map(|_| ()),
                                },
                            }
                        } else if msg
                            .from()
                            .filter(|user| user.id as i64 == user_id)
                            .is_some()
                        {
                            tg_bot.incorrect_request(user_id).await
                        } else {
                            Ok(())
                        }
                        .map_err(From::from)
                    }
                    UpdateKind::CallbackQuery(cb_query) => {
                        if let Some(cb_data) = &cb_query.data {
                            if let Some(msg) = &cb_query.message {
                                if let Some(page_num) = cb_data
                                    .strip_prefix("seltz::page::")
                                    .and_then(|x| x.parse::<usize>().ok())
                                {
                                    tg_bot
                                        .select_timezone_set_page(
                                            msg.chat_id(),
                                            page_num,
                                            msg.id,
                                        )
                                        .await
                                        .map_err(From::from)
                                } else if let Some(tz_name) =
                                    cb_data.strip_prefix("seltz::tz::")
                                {
                                    tg_bot
                                        .set_timezone(msg.chat_id(), tz_name)
                                        .await
                                        .map_err(From::from)
                                } else if let Some(page_num) = cb_data
                                    .strip_prefix("delrem::page::")
                                    .and_then(|x| x.parse::<usize>().ok())
                                {
                                    tg_bot
                                        .delete_reminder_set_page(
                                            msg.chat_id(),
                                            page_num,
                                            msg.id,
                                        )
                                        .await
                                        .map_err(From::from)
                                } else if let Some(rem_id) = cb_data
                                    .strip_prefix("delrem::alt::")
                                    .and_then(|x| x.parse::<u32>().ok())
                                {
                                    tg_bot
                                        .delete_reminder(
                                            msg.chat_id(),
                                            rem_id,
                                            msg.id,
                                        )
                                        .await
                                        .map_err(From::from)
                                } else if let Some(cron_rem_id) = cb_data
                                    .strip_prefix("delrem::cron_alt::")
                                    .and_then(|x| x.parse::<u32>().ok())
                                {
                                    tg_bot
                                        .delete_cron_reminder(
                                            msg.chat_id(),
                                            cron_rem_id,
                                            msg.id,
                                        )
                                        .await
                                        .map_err(From::from)
                                } else if let Some(page_num) = cb_data
                                    .strip_prefix("editrem::page::")
                                    .and_then(|x| x.parse::<usize>().ok())
                                {
                                    tg_bot
                                        .edit_reminder_set_page(
                                            msg.chat_id(),
                                            page_num,
                                            msg.id,
                                        )
                                        .await
                                        .map_err(From::from)
                                } else if let Some(rem_id) = cb_data
                                    .strip_prefix("editrem::alt::")
                                    .and_then(|x| x.parse::<u32>().ok())
                                {
                                    tg_bot
                                        .edit_reminder(msg.chat_id(), rem_id)
                                        .await
                                        .map_err(From::from)
                                } else if let Some(cron_rem_id) = cb_data
                                    .strip_prefix("editrem::cron_alt::")
                                    .and_then(|x| x.parse::<u32>().ok())
                                {
                                    tg_bot
                                        .edit_cron_reminder(
                                            msg.chat_id(),
                                            cron_rem_id,
                                        )
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
                    _ => Ok(()),
                },
                Err(err) => Err(From::from(err)),
            }
            .unwrap_or_else(|err| {
                dbg!(err);
            });
            database
        })
        .await;
}

#[tokio::main]
async fn main() {
    run().await;
}
