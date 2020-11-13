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

impl ToString for TgResponse {
    fn to_string(&self) -> String {
        let raw_text: String = match self {
            Self::SuccessInsert => "Remember that!".to_string(),
            Self::FailedInsert => "Failed to create a reminder...".to_string(),
            Self::IncorrectRequest => "Incorrect request!".to_string(),
            Self::QueryingError => "Error occured while querying reminders...".to_string(),
            Self::RemindersListHeader => "List of reminders:".to_string(),
            Self::SelectTimezone => "Select your timezone:".to_string(),
            Self::ChosenTimezone(tz_name) => format!("Selected timezone {}", tz_name),
            Self::NoChosenTimezone => "You've not selected timezone yet".to_string(),
            Self::FailedSetTimezone(tz_name) => format!("Failed to set timezone {}", tz_name),
            Self::FailedGetTimezone => format!("Failed to get timezone for reminder"),
        };
        escape(&raw_text)
    }
}

#[non_exhaustive]
struct ReminderRegexFields;

impl ReminderRegexFields {
    const DAY: &'static str = "day";
    const MONTH: &'static str = "month";
    const HOUR: &'static str = "hour";
    const MINUTE: &'static str = "minute";
    const SECOND: &'static str = "second";
    const DESCRIPTION: &'static str = "description";
}

impl ToString for db::Reminder {
    fn to_string(&self) -> String {
        match tz::get_user_timezone(self.user_id) {
            Ok(user_timezone) => {
                let time = user_timezone.from_utc_datetime(&self.time.naive_utc());
                let now = Utc::now().with_timezone(&user_timezone);
                let mut s = String::new();
                if time.date() != now.date() {
                    s = s
                        + &format!("{:02}", time.day())
                        + &escape(".")
                        + &format!("{:02}", time.month())
                        + " ";
                }
                s + &format!("{:02}", time.hour())
                    + ":"
                    + &format!("{:02}", time.minute())
                    + &escape(" <")
                    + &bold(&escape(&self.desc))
                    + &escape(">")
            }
            _ => TgResponse::FailedGetTimezone.to_string(),
        }
    }
}

impl ToString for db::CronReminder {
    fn to_string(&self) -> String {
        match tz::get_user_timezone(self.user_id) {
            Ok(user_timezone) => {
                let time = user_timezone.from_utc_datetime(&self.time.naive_utc());
                let now = Utc::now().with_timezone(&user_timezone);
                let mut s = String::new();
                if time.date() != now.date() {
                    s = s
                        + &format!("{:02}", time.day())
                        + &escape(".")
                        + &format!("{:02}", time.month())
                        + " ";
                }
                s + &format!("{:02}", time.hour())
                    + ":"
                    + &format!("{:02}", time.minute())
                    + &escape(" <")
                    + &bold(&escape(&self.desc))
                    + &escape("> [")
                    + &escape(&self.cron_expr)
                    + &escape("]")
            }
            _ => TgResponse::FailedGetTimezone.to_string(),
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
        static ref RE: Regex = Regex::new(&format!(
            r"^\s*((?P<{}>\d{{1,2}})(\.(?P<{}>\d{{2}}))?\s+)?(?P<{}>\d{{1,2}}):(?P<{}>\d{{2}})(:(?P<{}>\d{{2}}))?\s*(?P<{}>.*?)\s*$",
            ReminderRegexFields::DAY,
            ReminderRegexFields::MONTH,
            ReminderRegexFields::HOUR,
            ReminderRegexFields::MINUTE,
            ReminderRegexFields::SECOND,
            ReminderRegexFields::DESCRIPTION
        ))
        .unwrap();
    }
    match tz::get_user_timezone(msg.chat_id()) {
        Ok(user_timezone) => RE.captures(s).and_then(|caps| {
            let now = user_timezone.from_utc_datetime(&Utc::now().naive_utc());
            let get_field_by_name_or = |name, default| {
                caps.name(name)
                    .and_then(|x| x.as_str().parse().ok())
                    .unwrap_or(default)
            };
            let day = get_field_by_name_or(ReminderRegexFields::DAY, now.day());
            let month = get_field_by_name_or(ReminderRegexFields::MONTH, now.month());
            let hour = get_field_by_name_or(ReminderRegexFields::HOUR, now.hour());
            let minute = get_field_by_name_or(ReminderRegexFields::MINUTE, now.minute());
            let second = get_field_by_name_or(ReminderRegexFields::SECOND, now.minute());

            if !((0..24).contains(&hour) && (0..60).contains(&minute)) {
                return None;
            }

            let time = now
                .date()
                .with_day(day)
                .and_then(|x| x.with_month(month))
                .unwrap_or(now.date())
                .and_hms(hour, minute, second);
            Some(db::Reminder {
                id: 0,
                user_id: msg.chat_id(),
                time: time.with_timezone(&Utc),
                desc: caps[ReminderRegexFields::DESCRIPTION].to_string(),
                sent: false,
            })
        }),
        _ => None,
    }
}
