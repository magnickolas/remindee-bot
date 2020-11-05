use crate::db;
use crate::tz;

use chrono::offset::TimeZone;
use chrono::prelude::*;
use chrono::Utc;
use regex::Regex;
use teloxide::prelude::*;
use teloxide::types::ParseMode::MarkdownV2;
use teloxide::utils::markdown::{bold, escape};

pub enum TgResponse {
    SuccessInsert,
    FailedInsert,
    IncorrectRequest,
    QueryingError,
    RemindersListHeader,
    SelectTimezone,
    ChosenTimezone(String),
    NoChosenTimezone,
    FailedSetTimezone(String),
    FailedGetTimezone,
}

impl TgResponse {
    pub fn text(self) -> String {
        let raw_text: String = match self {
            TgResponse::SuccessInsert => "Remember that!".to_string(),
            TgResponse::FailedInsert => "Failed to create a reminder...".to_string(),
            TgResponse::IncorrectRequest => "Incorrect request!".to_string(),
            TgResponse::QueryingError => "Error occured while querying reminders...".to_string(),
            TgResponse::RemindersListHeader => "List of reminders:".to_string(),
            TgResponse::SelectTimezone => "Select your timezone:".to_string(),
            TgResponse::ChosenTimezone(tz_name) => format!("Selected timezone {}", tz_name),
            TgResponse::NoChosenTimezone => "You've not selected timezone yet".to_string(),
            TgResponse::FailedSetTimezone(tz_name) => format!("Failed to set timezone {}", tz_name),
            TgResponse::FailedGetTimezone => format!("Failed to get timezone for reminder"),
        };
        escape(&raw_text)
    }
}

impl ToString for db::Reminder {
    fn to_string(&self) -> String {
        match tz::get_user_timezone(self.user_id) {
            Ok(user_timezone) => {
                let time = user_timezone.from_utc_datetime(&self.time.naive_utc());
                format!("{:02}", time.hour())
                    + ":"
                    + &format!("{:02}", time.minute())
                    + &escape(" <")
                    + &bold(&escape(&self.desc))
                    + &escape(">")
            }
            _ => TgResponse::FailedGetTimezone.text(),
        }
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
    match tz::get_user_timezone(msg.chat_id()) {
        Ok(user_timezone) => RE.captures(s).map(|caps| {
            let now = user_timezone.from_utc_datetime(&Utc::now().naive_utc());
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
        }),
        _ => None,
    }
}
