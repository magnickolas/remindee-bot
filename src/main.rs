#[macro_use]
extern crate lazy_static;

mod db;
mod tg;
mod tz;

use async_std::task;
use std::time::Duration;
use teloxide::dispatching::update_listeners::polling_default;
use teloxide::prelude::*;
use teloxide::types::{
    ChatId, ChatOrInlineMessage, InlineKeyboardButton, InlineKeyboardButtonKind,
    InlineKeyboardMarkup, UpdateKind,
};
use tg::TgResponse;
use tokio::runtime::Handle;

async fn reminders_pooling(bot: &Bot) {
    loop {
        let reminders = db::get_active_reminders().unwrap();
        for reminder in reminders {
            match tg::send_message(&reminder.to_string(), &bot, reminder.user_id).await {
                Ok(_) => db::mark_reminder_as_sent(&reminder).unwrap_or_else(|err| {
                    dbg!(err);
                }),
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
                            InlineKeyboardButtonKind::CallbackData("tz::".to_string() + tz_name),
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
            InlineKeyboardButtonKind::CallbackData("page::".to_string() + &(num - 1).to_string()),
        ))
    }
    if !last_page {
        move_buttons.push(InlineKeyboardButton::new(
            "➡️",
            InlineKeyboardButtonKind::CallbackData("page::".to_string() + &(num + 1).to_string()),
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
    db::create_user_timezone_table().unwrap();

    let handle = Handle::current();

    let bot_clone = bot.clone();
    handle.spawn(async move { reminders_pooling(&bot_clone).await });

    updater
        .for_each(|update| async {
            match update {
                Ok(update) => match update.kind {
                    UpdateKind::Message(msg) => match msg.text() {
                        Some(text) => match text {
                            "list" | "/list" => {
                                let text = db::get_pending_user_reminders(&msg)
                                    .map(|v| {
                                        vec![TgResponse::RemindersListHeader.to_string()]
                                            .into_iter()
                                            .chain(v.into_iter().map(|x| x.to_string()))
                                            .collect::<Vec<String>>()
                                            .join("\n")
                                    })
                                    .unwrap_or(TgResponse::QueryingError.to_string());
                                tg::send_message(&text, &bot, msg.chat_id())
                                    .await
                                    .unwrap_or_else({
                                        |err| {
                                            dbg!(err);
                                        }
                                    });
                            }
                            "tz" | "/tz" | "timezone" | "/timezone" => {
                                bot.send_message(
                                    msg.chat_id(),
                                    TgResponse::SelectTimezone.to_string(),
                                )
                                .reply_markup(get_markup_for_page_idx(0))
                                .send()
                                .await
                                .map(|_| ())
                                .unwrap_or_else({
                                    |err| {
                                        dbg!(err);
                                    }
                                });
                            }
                            "mytz" | "/mytz" | "mytimezone" | "/mytimezone" => {
                                let response = match db::get_user_timezone_name(msg.chat_id()) {
                                    Ok(tz_name) => TgResponse::ChosenTimezone(tz_name),
                                    Err(err) => {
                                        dbg!(err);
                                        TgResponse::NoChosenTimezone
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
                            text => match tg::parse_req(text, &msg) {
                                Some(reminder) => {
                                    let response = match db::insert_reminder(&reminder) {
                                        Ok(_) => TgResponse::SuccessInsert,
                                        Err(err) => {
                                            dbg!(err);
                                            TgResponse::FailedInsert
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
                                None => match msg.from() {
                                    Some(user) if user.id as i64 == msg.chat_id() => {
                                        let response =
                                            if tz::get_user_timezone(msg.chat_id()).is_err() {
                                                TgResponse::NoChosenTimezone
                                            } else {
                                                TgResponse::IncorrectRequest
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
                                    _ => {}
                                },
                            },
                        },
                        None => {}
                    },
                    UpdateKind::CallbackQuery(cb_query) => {
                        if let Some(cb_data) = &cb_query.data {
                            if let Some(page_num_str) = cb_data.strip_prefix("page::") {
                                let page_num = page_num_str.parse::<usize>().unwrap();
                                if let Some(msg) = cb_query.message {
                                    bot.edit_message_reply_markup(ChatOrInlineMessage::Chat {
                                        chat_id: ChatId::Id(msg.chat_id()),
                                        message_id: msg.id,
                                    })
                                    .reply_markup(get_markup_for_page_idx(page_num))
                                    .send()
                                    .await
                                    .map(|_| ())
                                    .unwrap_or_else(|err| {
                                        dbg!(err);
                                    });
                                }
                            } else if let Some(tz_name) = cb_data.strip_prefix("tz::") {
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
                            }
                        }
                        dbg!("{}", &cb_query.data);
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
