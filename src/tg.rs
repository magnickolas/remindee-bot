use crate::db;

use chrono::offset::{FixedOffset, TimeZone};
use chrono::prelude::*;
use chrono::Utc;
use regex::Regex;
use teloxide::prelude::*;
use teloxide::types::ParseMode::MarkdownV2;
use teloxide::utils::markdown::{bold, escape};

pub enum TgResponse {
    SuccessInsert,
    IncorrectRequest,
    QueryingError,
    RemindersListHeader,
}

impl TgResponse {
    pub fn text(self) -> String {
        let raw_text = match self {
            TgResponse::SuccessInsert => "Remember that!",
            TgResponse::IncorrectRequest => "Incorrect request!",
            TgResponse::QueryingError => "Error occured while querying reminders...",
            TgResponse::RemindersListHeader => "List of reminders:",
        };
        escape(raw_text)
    }
}

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

pub async fn send_message(text: &String, bot: &Bot, user_id: i64) -> Result<(), RequestError> {
    bot.send_message(user_id, text)
        .parse_mode(MarkdownV2)
        .send()
        .await
        .map(|_| ())
}

pub fn parse_req(s: &str, msg: &Message) -> Option<db::Reminder> {
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
