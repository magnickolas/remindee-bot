#[macro_use]
extern crate lazy_static;

mod db;
mod err;
mod tg;
mod tz;

use async_std::task;
use chrono::Utc;
use cron_parser::parse as parse_cron;
use std::panic;
use std::time::Duration;
use teloxide::dispatching::update_listeners::polling_default;
use teloxide::prelude::*;
use teloxide::types::{
    InlineKeyboardButton, InlineKeyboardButtonKind, InlineKeyboardMarkup, UpdateKind,
};
use tg::TgResponse;
use tokio::runtime::Handle;

async fn reminders_pooling(bot: &Bot) {
    loop {
        let reminders = db::get_active_reminders().unwrap();
        for reminder in reminders {
            match tg::send_message(&reminder.to_string(), &bot, reminder.user_id).await {
                Ok(_) => db::mark_reminder_as_sent(reminder.id).unwrap_or_else(|err| {
                    dbg!(err);
                }),
                Err(err) => {
                    dbg!(err);
                }
            }
        }
        let cron_reminders = db::get_active_cron_reminders().unwrap();
        for cron_reminder in cron_reminders {
            match tg::send_message(&cron_reminder.to_string(), &bot, cron_reminder.user_id).await {
                Ok(_) => {
                    db::mark_cron_reminder_as_sent(cron_reminder.id).unwrap_or_else(|err| {
                        dbg!(err);
                    });
                    let new_time =
                        tz::get_user_timezone(cron_reminder.user_id).and_then(|timezone| {
                            panic::catch_unwind(|| {
                                parse_cron(
                                    &cron_reminder.cron_expr,
                                    &Utc::now().with_timezone(&timezone),
                                )
                                .map(|time| time.with_timezone(&Utc))
                                .map_err(From::from)
                            })
                            .unwrap_or(Err(err::Error::CronPanic))
                        });
                    match new_time {
                        Ok(new_time) => {
                            let new_cron_reminder = db::CronReminder {
                                time: new_time,
                                ..cron_reminder
                            };
                            db::insert_cron_reminder(&new_cron_reminder).unwrap_or_else(|err| {
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

fn get_markup_for_page_idx(num: usize) -> InlineKeyboardMarkup {
    let mut markup = InlineKeyboardMarkup::default();
    let mut last_page: bool = false;
    if let Some(tz_names) = tz::get_tz_names_for_page_idx(num) {
        for chunk in tz_names.chunks(2) {
            markup = markup.append_row(
                chunk
                    .to_vec()
                    .into_iter()
                    .map(|tz_name| {
                        InlineKeyboardButton::new(
                            tz_name,
                            InlineKeyboardButtonKind::CallbackData(
                                "seltz::tz::".to_string() + tz_name,
                            ),
                        )
                    })
                    .collect(),
            );
        }
    } else {
        last_page = true;
    }
    let mut move_buttons = vec![];
    if num > 0 {
        move_buttons.push(InlineKeyboardButton::new(
            "⬅️",
            InlineKeyboardButtonKind::CallbackData(
                "seltz::page::".to_string() + &(num - 1).to_string(),
            ),
        ))
    }
    if !last_page {
        move_buttons.push(InlineKeyboardButton::new(
            "➡️",
            InlineKeyboardButtonKind::CallbackData(
                "seltz::page::".to_string() + &(num + 1).to_string(),
            ),
        ))
    }
    markup.append_row(move_buttons)
}

fn get_markup_for_reminders_page_deletion(num: usize, user_id: i64) -> InlineKeyboardMarkup {
    let mut markup = InlineKeyboardMarkup::default();
    let mut last_rem_page: bool = false;
    let mut last_cron_rem_page: bool = false;
    if let Some(reminders) = db::get_pending_user_reminders(user_id)
        .ok()
        .as_ref()
        .and_then(|rems| rems.chunks(45).into_iter().nth(num))
    {
        for chunk in reminders.chunks(1) {
            markup = markup.append_row(
                chunk
                    .to_vec()
                    .into_iter()
                    .map(|rem| {
                        InlineKeyboardButton::new(
                            rem.to_unescaped_string(),
                            InlineKeyboardButtonKind::CallbackData(
                                "delrem::del::".to_string() + &rem.id.to_string(),
                            ),
                        )
                    })
                    .collect(),
            );
        }
    } else {
        last_rem_page = true;
    }
    if let Some(cron_reminders) = db::get_pending_user_cron_reminders(user_id)
        .ok()
        .as_ref()
        .and_then(|cron_rems| cron_rems.chunks(45).into_iter().nth(num))
    {
        for chunk in cron_reminders.chunks(1) {
            markup = markup.append_row(
                chunk
                    .to_vec()
                    .into_iter()
                    .map(|cron_rem| {
                        InlineKeyboardButton::new(
                            cron_rem.to_unescaped_string(),
                            InlineKeyboardButtonKind::CallbackData(
                                "delrem::cron_del::".to_string() + &cron_rem.id.to_string(),
                            ),
                        )
                    })
                    .collect(),
            );
        }
    } else {
        last_cron_rem_page = true;
    }
    let mut move_buttons = vec![];
    if num > 0 {
        move_buttons.push(InlineKeyboardButton::new(
            "⬅️",
            InlineKeyboardButtonKind::CallbackData(
                "delrem::page::".to_string() + &(num - 1).to_string(),
            ),
        ))
    }
    if !last_rem_page || !last_cron_rem_page {
        move_buttons.push(InlineKeyboardButton::new(
            "➡️",
            InlineKeyboardButtonKind::CallbackData(
                "delrem::page::".to_string() + &(num + 1).to_string(),
            ),
        ))
    }
    markup.append_row(move_buttons)
}

async fn run() {
    teloxide::enable_logging!();
    log::info!("Starting remindee bot!");

    let bot = Bot::from_env();
    let updater = polling_default(bot.clone());

    db::create_reminder_table().unwrap();
    db::create_cron_reminder_table().unwrap();
    db::create_user_timezone_table().unwrap();

    let handle = Handle::current();

    let bot_clone = bot.clone();
    handle.spawn(async move { reminders_pooling(&bot_clone).await });

    updater
        .for_each(|update| async {
            match update {
                Ok(update) => match update.kind {
                    UpdateKind::Message(msg) => {
                        let user_id = msg.chat_id();
                        match msg.text() {
                            Some(text) => match text {
                                "/start" => {
                                    tg::send_message(&TgResponse::Hello.to_string(), &bot, user_id)
                                        .await
                                        .unwrap_or_else({
                                            |err| {
                                                dbg!(err);
                                            }
                                        });
                                }
                                "list" | "/list" => {
                                    let mut text = db::get_pending_user_reminders(user_id)
                                        .map(|v| {
                                            vec![TgResponse::RemindersListHeader.to_string()]
                                                .into_iter()
                                                .chain(v.into_iter().map(|x| x.to_string()))
                                                .collect::<Vec<String>>()
                                                .join("\n")
                                        })
                                        .unwrap_or(TgResponse::QueryingError.to_string());
                                    text = db::get_pending_user_cron_reminders(user_id)
                                        .map(|v| {
                                            vec![text]
                                                .into_iter()
                                                .chain(v.into_iter().map(|x| x.to_string()))
                                                .collect::<Vec<String>>()
                                                .join("\n")
                                        })
                                        .unwrap_or(TgResponse::QueryingError.to_string());
                                    tg::send_message(&text, &bot, user_id)
                                        .await
                                        .unwrap_or_else({
                                            |err| {
                                                dbg!(err);
                                            }
                                        });
                                }
                                "tz" | "/tz" | "timezone" | "/timezone" => {
                                    tg::send_markup(
                                        &TgResponse::SelectTimezone.to_string(),
                                        get_markup_for_page_idx(0),
                                        &bot,
                                        user_id,
                                    )
                                    .await
                                    .unwrap_or_else({
                                        |err| {
                                            dbg!(err);
                                        }
                                    });
                                }
                                "mytz" | "/mytz" | "mytimezone" | "/mytimezone" => {
                                    let response = match db::get_user_timezone_name(user_id) {
                                        Ok(tz_name) => TgResponse::ChosenTimezone(tz_name),
                                        Err(err) => {
                                            dbg!(err);
                                            TgResponse::NoChosenTimezone
                                        }
                                    };
                                    tg::send_message(&response.to_string(), &bot, user_id)
                                        .await
                                        .unwrap_or_else({
                                            |err| {
                                                dbg!(err);
                                            }
                                        });
                                }
                                "del" | "/del" | "delete" | "/delete" => {
                                    tg::send_markup(
                                        &TgResponse::ChooseDeleteReminder.to_string(),
                                        get_markup_for_reminders_page_deletion(0, user_id),
                                        &bot,
                                        user_id,
                                    )
                                    .await
                                    .unwrap_or_else({
                                        |err| {
                                            dbg!(err);
                                        }
                                    });
                                }
                                "/commands" => {
                                    tg::send_message(
                                        &TgResponse::CommandsHelp.to_string(),
                                        &bot,
                                        user_id,
                                    )
                                    .await
                                    .unwrap_or_else({
                                        |err| {
                                            dbg!(err);
                                        }
                                    });
                                }
                                text => {
                                    if let Some(reminder) = tg::parse_req(text, user_id) {
                                        let response = match db::insert_reminder(&reminder) {
                                            Ok(_) => TgResponse::SuccessInsert,
                                            Err(err) => {
                                                dbg!(err);
                                                TgResponse::FailedInsert
                                            }
                                        };

                                        tg::send_message(&response.to_string(), &bot, user_id)
                                            .await
                                            .unwrap_or_else({
                                                |err| {
                                                    dbg!(err);
                                                }
                                            });
                                    } else if let Ok((cron_expr, time)) = {
                                        let cron_fields: Vec<&str> =
                                            text.split_whitespace().take(5).collect();
                                        if cron_fields.len() < 5 {
                                            Err(err::Error::CronFewFields)
                                        } else {
                                            let cron_expr = cron_fields.join(" ");
                                            tz::get_user_timezone(user_id).and_then(|timezone| {
                                                let time = parse_cron(
                                                    &cron_expr,
                                                    &Utc::now().with_timezone(&timezone),
                                                )?
                                                .with_timezone(&Utc);
                                                Ok((cron_expr, time))
                                            })
                                        }
                                    } {
                                        let response =
                                            match db::insert_cron_reminder(&db::CronReminder {
                                                id: 0,
                                                user_id,
                                                cron_expr: cron_expr.clone(),
                                                time,
                                                desc: text
                                                    .strip_prefix(&(cron_expr.to_string() + " "))
                                                    .unwrap_or("")
                                                    .to_string(),
                                                sent: false,
                                            }) {
                                                Ok(_) => TgResponse::SuccessInsert,
                                                Err(err) => {
                                                    dbg!(err);
                                                    TgResponse::FailedInsert
                                                }
                                            };

                                        tg::send_message(&response.to_string(), &bot, user_id)
                                            .await
                                            .unwrap_or_else({
                                                |err| {
                                                    dbg!(err);
                                                }
                                            });
                                    } else {
                                        match msg.from() {
                                            Some(user) if user.id as i64 == user_id => {
                                                let response =
                                                    if tz::get_user_timezone(user_id).is_err() {
                                                        TgResponse::NoChosenTimezone
                                                    } else {
                                                        TgResponse::IncorrectRequest
                                                    };
                                                tg::send_message(
                                                    &response.to_string(),
                                                    &bot,
                                                    user_id,
                                                )
                                                .await
                                                .unwrap_or_else({
                                                    |err| {
                                                        dbg!(err);
                                                    }
                                                });
                                            }
                                            _ => {}
                                        }
                                    }
                                }
                            },
                            None => {}
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
