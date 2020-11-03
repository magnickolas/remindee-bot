#[macro_use]
extern crate lazy_static;

use async_std::task;
use chrono::offset::{FixedOffset, TimeZone};
use chrono::prelude::*;
use chrono::{DateTime, Utc};
use directories::BaseDirs;
use regex::Regex;
use rusqlite::{params, Connection, Result, NO_PARAMS};
use std::time::Duration;
use teloxide::dispatching::update_listeners::polling_default;
use teloxide::prelude::*;
use teloxide::types::Message;
use teloxide::types::ParseMode::MarkdownV2;
use teloxide::types::UpdateKind;
use teloxide::utils::markdown::{bold, escape};
use tokio::runtime::Handle;

#[derive(Debug)]
struct Reminder {
    id: u32,
    user_id: i64,
    time: DateTime<Utc>,
    desc: String,
    sent: bool,
}

impl ToString for Reminder {
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

fn parse_req(s: &str, msg: &Message) -> Option<Reminder> {
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
        Reminder {
            id: 0,
            user_id: msg.chat_id(),
            time: time.with_timezone(&Utc),
            desc: caps["desc"].to_string(),
            sent: false,
        }
    })
}

fn get_db_connection() -> Result<Connection> {
    let base_dirs = BaseDirs::new().unwrap();
    Connection::open(base_dirs.data_dir().join("remindee_db.sqlite"))
}

fn create_reminder_table() -> Result<()> {
    let conn = get_db_connection()?;
    conn.execute(
        "create table if not exists reminder (
             id         integer primary key,
             user_id    integer not null,
             time       timestamp not null,
             desc       text not null,
             sent       boolean not null
        )",
        params![],
    )?;
    Ok(())
}

fn insert_reminder(rem: &Reminder) -> Result<()> {
    let conn = get_db_connection()?;
    conn.execute(
        "insert into reminder (user_id, time, desc, sent) values (?1, ?2, ?3, ?4)",
        params![rem.user_id, rem.time, rem.desc, rem.sent],
    )?;
    Ok(())
}

fn mark_reminder_as_sent(rem: &Reminder) -> Result<()> {
    let conn = get_db_connection()?;
    conn.execute("update reminder set sent=true where id=?1", params![rem.id])?;
    Ok(())
}

fn get_active_reminders() -> Result<Vec<Reminder>> {
    let conn = get_db_connection()?;
    let mut stmt = conn.prepare(
        "select id, user_id, time, desc, sent
        from reminder
        where sent=false and datetime(time) < datetime('now')",
    )?;
    let rows = stmt.query_map(NO_PARAMS, |row| {
        Ok(Reminder {
            id: row.get(0)?,
            user_id: row.get(1)?,
            time: row.get(2)?,
            desc: row.get(3)?,
            sent: row.get(4)?,
        })
    })?;
    let mut reminders = Vec::new();
    for reminder in rows {
        reminders.push(reminder?);
    }
    Ok(reminders)
}

fn get_active_user_reminders(msg: &Message) -> Result<Vec<Reminder>> {
    let conn = get_db_connection()?;
    let mut stmt = conn.prepare(
        "select id, user_id, time, desc, sent
            from reminder
            where user_id=?1 and datetime(time) >= datetime('now')",
    )?;
    let rows = stmt.query_map(params![msg.chat_id()], |row| {
        Ok(Reminder {
            id: row.get(0)?,
            user_id: row.get(1)?,
            time: row.get(2)?,
            desc: row.get(3)?,
            sent: row.get(4)?,
        })
    })?;
    let mut reminders = Vec::new();
    for reminder in rows {
        reminders.push(reminder?);
    }
    Ok(reminders)
}

async fn reminders_pooling(bot: &Bot) {
    loop {
        let reminders = get_active_reminders().unwrap();
        for reminder in reminders {
            send_message(&reminder.to_string(), &bot, reminder.user_id).await;
            mark_reminder_as_sent(&reminder).unwrap()
        }
        task::sleep(Duration::from_secs(1)).await;
    }
}

async fn run() {
    teloxide::enable_logging!();
    log::info!("Starting remindee bot!");

    let bot = Bot::from_env();
    let updater = polling_default(bot.clone());

    create_reminder_table().unwrap();

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
                                let text = get_active_user_reminders(&msg)
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
                                    let res = insert_reminder(&reminder);
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
                                None => {
                                    // send_message(
                                    //     &escape("Incorrect request\\!"),
                                    //     &bot,
                                    //     msg.chat_id(),
                                    // )
                                    // .await
                                }
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
