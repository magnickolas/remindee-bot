#[macro_use]
extern crate lazy_static;

mod db;

use async_std::task;
use chrono::offset::{FixedOffset, TimeZone};
use chrono::prelude::*;
use chrono::Utc;
use regex::Regex;
use std::time::Duration;
use teloxide::dispatching::update_listeners::polling_default;
use teloxide::prelude::*;
use teloxide::types::Message;
use teloxide::types::ParseMode::MarkdownV2;
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

async fn send_message(text: &String, bot: &Bot, user_id: i64) {
    bot.send_message(user_id, text)
        .parse_mode(MarkdownV2)
        .send()
        .await
        .map(|_| ())
        .unwrap_or_else(|err| {
            dbg!(err);
        })
}

fn parse_req(s: &str, msg: &Message) -> Option<db::Reminder> {
    lazy_static! {
        static ref RE: Regex =
            Regex::new(r"^(?P<hour>\d{1,2}):(?P<minutes>\d{2})\s*(?P<desc>.*?)\s*$").unwrap();
    }
    RE.captures(s).map(|caps| {
        //TODO remove fixed offset
        let now = FixedOffset::east(3 * 3600).from_utc_datetime(&Utc::now().naive_utc());
        let time = now.date().and_hms(
            caps["hour"].to_string().parse().unwrap(),
            caps["minutes"].to_string().parse().unwrap(),
            0,
        );
        db::Reminder {
            id: 0,
            user_id: msg.chat_id(),
            time: time.with_timezone(&Utc),
            desc: caps["desc"].to_string(),
            sent: false,
        }
    })
}

async fn reminders_pooling(bot: &Bot) {
    loop {
        let reminders = db::get_active_reminders().unwrap();
        for reminder in reminders {
            send_message(&reminder.to_string(), &bot, reminder.user_id).await;
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
                                send_message(&text, &bot, msg.chat_id()).await
                            }
                            text => match parse_req(text, &msg) {
                                Some(reminder) => {
                                    let res = db::insert_reminder(&reminder);
                                    if res.is_ok() {
                                        send_message(&escape("Remember that!"), &bot, msg.chat_id())
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
                                        send_message(
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
