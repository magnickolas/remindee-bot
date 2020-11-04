#[macro_use]
extern crate lazy_static;

mod db;
mod tg;

use async_std::task;
use std::time::Duration;
use teloxide::dispatching::update_listeners::polling_default;
use teloxide::prelude::*;
use teloxide::types::UpdateKind;
use tg::TgResponse;
use tokio::runtime::Handle;

async fn reminders_pooling(bot: &Bot) {
    loop {
        let reminders = db::get_active_reminders().unwrap();
        for reminder in reminders {
            tg::send_message(&reminder.to_string(), &bot, reminder.user_id)
                .await
                .map_err(|err| {
                    dbg!(err);
                })
                .and_then(|_| {
                    db::mark_reminder_as_sent(&reminder).map_err(|err| {
                        dbg!(err);
                    })
                })
                .unwrap_or_default();
        }
        task::sleep(Duration::from_secs(1)).await;
    }
}

async fn run() {
    teloxide::enable_logging!();
    log::info!("Starting remindee bot!");

    let bot = Bot::from_env();
    let updater = polling_default(bot.clone());

    db::create_reminder_table().unwrap();

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
                                        vec![TgResponse::RemindersListHeader.text()]
                                            .into_iter()
                                            .chain(v.into_iter().map(|x| x.to_string()))
                                            .collect::<Vec<String>>()
                                            .join("\n")
                                    })
                                    .unwrap_or(TgResponse::QueryingError.text());
                                tg::send_message(&text, &bot, msg.chat_id())
                                    .await
                                    .unwrap_or_else({
                                        |err| {
                                            dbg!(err);
                                        }
                                    });
                            }
                            text => match tg::parse_req(text, &msg) {
                                Some(reminder) => {
                                    let res = db::insert_reminder(&reminder);
                                    if res.is_ok() {
                                        tg::send_message(
                                            &TgResponse::SuccessInsert.text(),
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
                                    res.unwrap_or_else({
                                        |err| {
                                            dbg!(err);
                                        }
                                    });
                                }
                                None => match msg.from() {
                                    Some(user) if user.id as i64 == msg.chat_id() => {
                                        tg::send_message(
                                            &TgResponse::IncorrectRequest.text(),
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
