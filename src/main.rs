#[macro_use]
extern crate lazy_static;

mod db;
mod tg;

use async_std::task;
use chrono::offset::FixedOffset;
use chrono::prelude::*;
use std::time::Duration;
use teloxide::dispatching::update_listeners::polling_default;
use teloxide::prelude::*;
use teloxide::types::UpdateKind;
use teloxide::utils::markdown::{bold, escape};
use tokio::runtime::Handle;

impl ToString for db::Reminder {
    fn to_string(&self) -> String {
        //TODO remove fixed offset
        let time = FixedOffset::east(3 * 3600).from_utc_datetime(&self.time.naive_utc());
        format!("{:02}", time.hour())
            + ":"
            + &format!("{:02}", time.minute())
            + &escape(" <")
            + &bold(&escape(&self.desc))
            + &escape(">")
    }
}

async fn reminders_pooling(bot: &Bot) {
    loop {
        let reminders = db::get_active_reminders().unwrap();
        for reminder in reminders {
            tg::send_message(&reminder.to_string(), &bot, reminder.user_id).await;
            db::mark_reminder_as_sent(&reminder).unwrap()
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
                                        vec!["List of reminders:".to_string()]
                                            .into_iter()
                                            .chain(v.into_iter().map(|x| x.to_string()))
                                            .collect::<Vec<String>>()
                                            .join("\n")
                                    })
                                    .unwrap_or(
                                        "Error occured while querying reminders...".to_string(),
                                    );
                                tg::send_message(&text, &bot, msg.chat_id()).await
                            }
                            text => match tg::parse_req(text, &msg) {
                                Some(reminder) => {
                                    let res = db::insert_reminder(&reminder);
                                    if res.is_ok() {
                                        tg::send_message(
                                            &escape("Remember that!"),
                                            &bot,
                                            msg.chat_id(),
                                        )
                                        .await
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
                                            &escape("Incorrect request!"),
                                            &bot,
                                            msg.chat_id(),
                                        )
                                        .await
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
