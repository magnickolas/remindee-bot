use crate::controller::{TgCallbackController, TgMessageController};
use crate::db::Database;
use crate::entity::{cron_reminder, reminder};
use crate::err::Error;
use crate::generic_reminder::GenericReminder;
use crate::tg::send_message;
use crate::tz::get_user_timezone;
use async_once::AsyncOnce;
use async_std::task;
use chrono::Utc;
use chrono_tz::Tz;
use cron_parser::parse as parse_cron;
use sea_orm::{ActiveModelTrait, ActiveValue::NotSet, IntoActiveModel};
use std::time::Duration;
use teloxide::prelude::*;
use teloxide::types::MessageId;

async fn format_reminder<T: ActiveModelTrait + GenericReminder>(
    reminder: &T,
    user_timezone: Tz,
) -> Result<String, Error> {
    Ok(match reminder.user_id() {
        Some(user_id) if reminder.chat_id().is_group() => {
            reminder.to_string_with_mention(user_timezone, user_id.0 as i64)
        }
        _ => reminder.to_string(user_timezone),
    })
}

async fn format_cron_reminder(
    reminder: &cron_reminder::Model,
    next_reminder: &Option<cron_reminder::Model>,
    user_timezone: Tz,
) -> Result<String, Error> {
    let formatted_reminder =
        format_reminder(&reminder.clone().into_active_model(), user_timezone)
            .await?;
    Ok(match next_reminder {
        Some(next_reminder) => format!(
            "{}\n\nNext time → {}",
            formatted_reminder,
            next_reminder
                .clone()
                .into_active_model()
                .serialize_time(user_timezone)
        ),
        None => formatted_reminder,
    })
}

async fn send_reminder(
    reminder: &reminder::Model,
    user_timezone: Tz,
    bot: &Bot,
) -> Result<(), Error> {
    let text =
        format_reminder(&reminder.clone().into_active_model(), user_timezone)
            .await?;
    send_message(&text, bot, ChatId(reminder.chat_id))
        .await
        .map_err(From::from)
}

async fn send_cron_reminder(
    reminder: &cron_reminder::Model,
    next_reminder: &Option<cron_reminder::Model>,
    user_timezone: Tz,
    bot: &Bot,
) -> Result<(), Error> {
    let text =
        format_cron_reminder(reminder, next_reminder, user_timezone).await?;
    send_message(&text, bot, ChatId(reminder.chat_id))
        .await
        .map_err(From::from)
}

/// Iterate every second all reminders and send notifications if time's come
async fn reminders_pooling(db: &Database, bot: Bot) {
    loop {
        let reminders = db.get_active_reminders().await.unwrap();
        for reminder in reminders {
            if let Some(user_id) = reminder.user_id.map(|x| UserId(x as u64)) {
                if let Ok(Some(user_timezone)) =
                    get_user_timezone(db, user_id).await
                {
                    match send_reminder(&reminder, user_timezone, &bot).await {
                        Ok(()) => db
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
                    get_user_timezone(db, user_id).await
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
                    match send_cron_reminder(
                        &cron_reminder,
                        &new_cron_reminder,
                        user_timezone,
                        &bot,
                    )
                    .await
                    {
                        Ok(()) => {
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
    static ref DATABASE: AsyncOnce<Database> =
        AsyncOnce::new(async { Database::new().await.unwrap() });
}

pub async fn run() {
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

impl<'a> TgMessageController<'a> {
    pub async fn new(
        bot: &'a Bot,
        chat_id: ChatId,
        user_id: UserId,
        msg_id: MessageId,
    ) -> Result<TgMessageController<'a>, Error> {
        Ok(Self {
            db: DATABASE.get().await,
            bot,
            chat_id,
            user_id,
            msg_id,
        })
    }
}

impl<'a> TgCallbackController<'a> {
    pub async fn new(
        bot: &'a Bot,
        cb_query: &'a CallbackQuery,
    ) -> Result<TgCallbackController<'a>, Error> {
        let msg = cb_query
            .message
            .as_ref()
            .ok_or_else(|| Error::NoQueryMessage(cb_query.clone()))?;
        Ok(Self {
            msg_ctl: TgMessageController::new(
                bot,
                msg.chat.id,
                cb_query.from.id,
                msg.id,
            )
            .await?,
            cb_id: &cb_query.id,
        })
    }
}

async fn remove_bot_mention(bot: &Bot, text: &str) -> Result<String, Error> {
    let mention = bot.get_me().await?.mention();
    let mut parts = text.split_whitespace();
    let new_first_part =
        parts.next().map(|x| x.strip_suffix(&mention).unwrap_or(x));
    Ok(new_first_part
        .into_iter()
        .chain(parts)
        .collect::<Vec<_>>()
        .join(" "))
}

async fn message_handler(msg: Message, bot: Bot) -> Result<(), Error> {
    let ctl = TgMessageController::new(
        &bot,
        msg.chat.id,
        msg.from()
            .ok_or_else(|| Error::UserNotFound(msg.clone()))?
            .id,
        msg.id,
    )
    .await?;
    if let Some(text) = msg.text() {
        match remove_bot_mention(&bot, text).await?.as_str() {
            "/start" => ctl.start().await,
            "/list" => ctl.list().await,
            "/tz" | "/timezone" => ctl.choose_timezone().await,
            "/mytz" | "/mytimezone" => ctl.get_timezone().await,
            "/del" | "/delete" => ctl.start_delete().await,
            "/edit" => ctl.start_edit().await,
            "/pause" => ctl.start_pause().await,
            "/help" => ctl.list_commands().await,
            text => {
                let reminder_text =
                    text.strip_prefix("/set ").unwrap_or(text).trim();
                ctl.set_or_edit_reminder(reminder_text).await
            }
        }
    } else if ctl.chat_id.is_user() {
        ctl.incorrect_request().await
    } else {
        Ok(())
    }
    .map_err(From::from)
}

async fn callback_handler(
    cb_query: CallbackQuery,
    bot: Bot,
) -> Result<(), Error> {
    if let Some(cb_data) = &cb_query.data {
        let ctl = TgCallbackController::new(&bot, &cb_query).await?;
        let msg_ctl = &ctl.msg_ctl;
        if let Some(page_num) = cb_data
            .strip_prefix("seltz::page::")
            .and_then(|x| x.parse::<usize>().ok())
        {
            msg_ctl
                .select_timezone_set_page(page_num)
                .await
                .map_err(From::from)
        } else if let Some(tz_name) = cb_data.strip_prefix("seltz::tz::") {
            ctl.set_timezone(tz_name).await.map_err(From::from)
        } else if let Some(page_num) = cb_data
            .strip_prefix("delrem::page::")
            .and_then(|x| x.parse::<usize>().ok())
        {
            msg_ctl
                .delete_reminder_set_page(page_num)
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
            msg_ctl
                .edit_reminder_set_page(page_num)
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
        } else if let Some(page_num) = cb_data
            .strip_prefix("pauserem::page::")
            .and_then(|x| x.parse::<usize>().ok())
        {
            msg_ctl
                .pause_reminder_set_page(page_num)
                .await
                .map_err(From::from)
        } else if let Some(cron_rem_id) = cb_data
            .strip_prefix("pauserem::cron_rem_alt::")
            .and_then(|x| x.parse::<i64>().ok())
        {
            ctl.pause_cron_reminder(cron_rem_id)
                .await
                .map_err(From::from)
        } else {
            Err(Error::UnmatchedQuery(cb_query))
        }
    } else {
        Err(Error::NoQueryData(cb_query))
    }
}
