#[macro_use]
extern crate lazy_static;

mod controller;
mod db;
mod err;
mod tg;
mod tz;

use async_std::task;
use chrono::Utc;
use controller::get_markup_for_page_idx;
use controller::get_markup_for_reminders_page_deletion;
use cron_parser::parse as parse_cron;
use std::future::Future;
use std::time::Duration;
use teloxide::dispatching::update_listeners::polling_default;
use teloxide::prelude::*;
use teloxide::types::UpdateKind;
use tg::TgResponse;

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

pub async fn unwrap_endpoint<Fut>(f: Fut)
where
    Fut: Future<Output = Result<(), RequestError>>,
{
    f.await.unwrap_or_else(|err| {
        dbg!(err);
    });
}

async fn run() {
    teloxide::enable_logging!();
    log::info!("Starting remindee bot!");

    // Create necessary database tables if they do not exist
    db::create_reminder_table().unwrap();
    db::create_cron_reminder_table().unwrap();
    db::create_user_timezone_table().unwrap();

    let bot = Bot::from_env();
    let updater = polling_default(bot.clone());

    // Run a database polling loop checking pending reminders asynchronously
    tokio::spawn(reminders_pooling(bot.clone()));

    // Run a telegram polling loop waiting messages from users and responding to them
    updater
        .for_each(|update| async {
            match update {
                Ok(update) => match update.kind {
                    UpdateKind::Message(msg) => {
                        let user_id = msg.chat_id();
                        if let Some(text) = msg.text() {
                            match text {
                                "/start" => unwrap_endpoint(controller::start(&bot, user_id)).await,
                                "list" | "/list" => {
                                    unwrap_endpoint(controller::list(&bot, user_id)).await
                                }
                                "tz" | "/tz" | "timezone" | "/timezone" => {
                                    unwrap_endpoint(controller::choose_timezone(&bot, user_id))
                                        .await
                                }
                                "mytz" | "/mytz" | "mytimezone" | "/mytimezone" => {
                                    unwrap_endpoint(controller::get_timezone(&bot, user_id)).await
                                }
                                "del" | "/del" | "delete" | "/delete" => {
                                    unwrap_endpoint(controller::start_delete(&bot, user_id)).await
                                }
                                "/commands" => {
                                    unwrap_endpoint(controller::list_commands(&bot, user_id)).await
                                }
                                text => {
                                    unwrap_endpoint(controller::set_reminder(
                                        &text,
                                        &bot,
                                        user_id,
                                        msg.from().map(|user| user.id),
                                    ))
                                    .await
                                }
                            }
                        }
                    }
                    UpdateKind::CallbackQuery(cb_query) => {
                        if let Some(cb_data) = &cb_query.data {
                            if let Some(page_num_str) = cb_data.strip_prefix("seltz::page::") {
                                if let Ok(page_num) = page_num_str.parse::<usize>() {
                                    if let Some(msg) = cb_query.message {
                                        tg::edit_markup(
                                            get_markup_for_page_idx(page_num),
                                            &bot,
                                            msg.id,
                                            msg.chat_id(),
                                        )
                                        .await
                                        .unwrap_or_else({
                                            |err| {
                                                dbg!(err);
                                            }
                                        });
                                    }
                                }
                            } else if let Some(tz_name) = cb_data.strip_prefix("seltz::tz::") {
                                if let Some(msg) = cb_query.message {
                                    let response =
                                        match db::set_user_timezone_name(msg.chat_id(), tz_name) {
                                            Ok(_) => {
                                                TgResponse::ChosenTimezone(tz_name.to_string())
                                            }
                                            Err(err) => {
                                                dbg!(err);
                                                TgResponse::FailedSetTimezone(tz_name.to_string())
                                            }
                                        };
                                    tg::send_message(&response.to_string(), &bot, msg.chat_id())
                                        .await
                                        .unwrap_or_else({
                                            |err| {
                                                dbg!(err);
                                            }
                                        });
                                }
                            } else if let Some(page_num_str) =
                                cb_data.strip_prefix("delrem::page::")
                            {
                                if let Ok(page_num) = page_num_str.parse::<usize>() {
                                    if let Some(msg) = cb_query.message {
                                        tg::edit_markup(
                                            get_markup_for_reminders_page_deletion(
                                                page_num,
                                                msg.chat_id(),
                                            ),
                                            &bot,
                                            msg.id,
                                            msg.chat_id(),
                                        )
                                        .await
                                        .unwrap_or_else({
                                            |err| {
                                                dbg!(err);
                                            }
                                        });
                                    }
                                }
                            } else if let Some(rem_id_str) = cb_data.strip_prefix("delrem::del::") {
                                if let Ok(rem_id) = rem_id_str.parse::<u32>() {
                                    if let Some(msg) = cb_query.message {
                                        let response = match db::mark_reminder_as_sent(rem_id) {
                                            Ok(_) => TgResponse::SuccessDelete,
                                            Err(err) => {
                                                dbg!(err);
                                                TgResponse::FailedDelete
                                            }
                                        };
                                        tg::edit_markup(
                                            get_markup_for_reminders_page_deletion(
                                                0,
                                                msg.chat_id(),
                                            ),
                                            &bot,
                                            msg.id,
                                            msg.chat_id(),
                                        )
                                        .await
                                        .unwrap_or_else({
                                            |err| {
                                                dbg!(err);
                                            }
                                        });
                                        tg::send_message(
                                            &response.to_string(),
                                            &bot,
                                            msg.chat_id(),
                                        )
                                        .await
                                        .unwrap_or_else({
                                            |err| {
                                                dbg!(err);
                                            }
                                        });
                                    }
                                }
                            } else if let Some(cron_rem_id_str) =
                                cb_data.strip_prefix("delrem::cron_del::")
                            {
                                if let Ok(cron_rem_id) = cron_rem_id_str.parse::<u32>() {
                                    if let Some(msg) = cb_query.message {
                                        let response =
                                            match db::mark_cron_reminder_as_sent(cron_rem_id) {
                                                Ok(_) => TgResponse::SuccessDelete,
                                                Err(err) => {
                                                    dbg!(err);
                                                    TgResponse::FailedDelete
                                                }
                                            };
                                        tg::edit_markup(
                                            get_markup_for_reminders_page_deletion(
                                                0,
                                                msg.chat_id(),
                                            ),
                                            &bot,
                                            msg.id,
                                            msg.chat_id(),
                                        )
                                        .await
                                        .unwrap_or_else({
                                            |err| {
                                                dbg!(err);
                                            }
                                        });
                                        tg::send_message(
                                            &response.to_string(),
                                            &bot,
                                            msg.chat_id(),
                                        )
                                        .await
                                        .unwrap_or_else({
                                            |err| {
                                                dbg!(err);
                                            }
                                        });
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                },
                Err(error) => {
                    dbg!(error);
                }
            }
        })
        .await;
}

#[tokio::main]
async fn main() {
    run().await;
}
