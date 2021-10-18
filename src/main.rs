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

/// Iterate every second all reminders and send notifications if time's come
async fn reminders_pooling(bot: Bot) {
    loop {
        let reminders = db::get_active_reminders().unwrap();
        for reminder in reminders {
            match tg::send_message(
                &reminder.to_string(),
                &bot,
                reminder.user_id,
            )
            .await
            {
                Ok(_) => db::mark_reminder_as_sent(reminder.id).unwrap_or_else(
                    |err| {
                        dbg!(err);
                    },
                ),
                Err(err) => {
                    dbg!(err);
                }
            }
        }
        let cron_reminders = db::get_active_cron_reminders().unwrap();
        for cron_reminder in cron_reminders {
            match tg::send_message(
                &cron_reminder.to_string(),
                &bot,
                cron_reminder.user_id,
            )
            .await
            {
                Ok(_) => {
                    db::mark_cron_reminder_as_sent(cron_reminder.id)
                        .unwrap_or_else(|err| {
                            dbg!(err);
                        });
                    let new_time = tz::get_user_timezone(cron_reminder.user_id)
                        .and_then(|timezone| {
                            Ok(parse_cron(
                                &cron_reminder.cron_expr,
                                &Utc::now().with_timezone(&timezone),
                            )?
                            .with_timezone(&Utc))
                        });
                    match new_time {
                        Ok(new_time) => {
                            let new_cron_reminder = db::CronReminder {
                                time: new_time,
                                ..cron_reminder
                            };
                            db::insert_cron_reminder(&new_cron_reminder)
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
        task::sleep(Duration::from_secs(1)).await;
    }
}

fn set_token(token: &str) {
    std::env::set_var("TELOXIDE_TOKEN", token);
}

async fn run() {
    teloxide::enable_logging!();
    log::info!("Starting remindee bot!");

    // Create necessary database tables if they do not exist
    db::create_reminder_table().unwrap();
    db::create_cron_reminder_table().unwrap();
    db::create_user_timezone_table().unwrap();

    // Set token from an environment variable
    let token = std::env::var("BOT_TOKEN")
        .map_err(|err| {
            log::error!("{}", err);
        })
        .unwrap();
    set_token(&token);
    let bot = Bot::from_env();
    let updater = polling_default(bot.clone());

    // Run a database polling loop checking pending reminders asynchronously
    tokio::spawn(reminders_pooling(bot.clone()));

    // Run a telegram polling loop waiting messages from users and responding to them
    updater
        .await
        .as_stream()
        .for_each(|update| async {
            match update {
                Ok(update) => match update.kind {
                    UpdateKind::Message(msg) => {
                        let user_id = msg.chat_id();
                        if let Some(text) = msg.text() {
                            match text {
                                "/start" => {
                                    controller::start(&bot, user_id).await
                                }
                                "list" | "/list" => {
                                    controller::list(&bot, user_id).await
                                }
                                "tz" | "/tz" | "timezone" | "/timezone" => {
                                    controller::choose_timezone(&bot, user_id)
                                        .await
                                }
                                "mytz" | "/mytz" | "mytimezone"
                                | "/mytimezone" => {
                                    controller::get_timezone(&bot, user_id)
                                        .await
                                }
                                "del" | "/del" | "delete" | "/delete" => {
                                    controller::start_delete(&bot, user_id)
                                        .await
                                }
                                "edit" | "/edit" => {
                                    controller::start_edit(&bot, user_id).await
                                }
                                "/commands" => {
                                    controller::list_commands(&bot, user_id)
                                        .await
                                }
                                text => match (
                                    db::get_edit_reminder(user_id),
                                    db::get_edit_cron_reminder(user_id),
                                ) {
                                    (Ok(Some(edit_reminder)), _) => {
                                        controller::replace_reminder(
                                            text,
                                            &bot,
                                            user_id,
                                            edit_reminder.id,
                                            msg.from().map(|user| user.id),
                                        )
                                        .await
                                    }
                                    (_, Ok(Some(edit_cron_reminder))) => {
                                        controller::replace_cron_reminder(
                                            text,
                                            &bot,
                                            user_id,
                                            edit_cron_reminder.id,
                                            msg.from().map(|user| user.id),
                                        )
                                        .await
                                    }
                                    _ => controller::set_reminder(
                                        text,
                                        &bot,
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
                            controller::incorrect_request(&bot, user_id).await
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
                                    controller::select_timezone_set_page(
                                        &bot,
                                        msg.chat_id(),
                                        page_num,
                                        msg.id,
                                    )
                                    .await
                                    .map_err(From::from)
                                } else if let Some(tz_name) =
                                    cb_data.strip_prefix("seltz::tz::")
                                {
                                    controller::set_timezone(
                                        &bot,
                                        msg.chat_id(),
                                        tz_name,
                                    )
                                    .await
                                    .map_err(From::from)
                                } else if let Some(page_num) = cb_data
                                    .strip_prefix("delrem::page::")
                                    .and_then(|x| x.parse::<usize>().ok())
                                {
                                    controller::delete_reminder_set_page(
                                        &bot,
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
                                    controller::delete_reminder(
                                        &bot,
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
                                    controller::delete_cron_reminder(
                                        &bot,
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
                                    controller::edit_reminder_set_page(
                                        &bot,
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
                                    controller::edit_reminder(
                                        &bot,
                                        msg.chat_id(),
                                        rem_id,
                                    )
                                    .await
                                    .map_err(From::from)
                                } else if let Some(cron_rem_id) = cb_data
                                    .strip_prefix("editrem::cron_alt::")
                                    .and_then(|x| x.parse::<u32>().ok())
                                {
                                    controller::edit_cron_reminder(
                                        &bot,
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
            })
        })
        .await;
}

#[tokio::main]
async fn main() {
    run().await;
}
