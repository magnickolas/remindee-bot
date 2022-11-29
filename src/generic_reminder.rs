use crate::entity::{cron_reminder, reminder};
use chrono::prelude::*;
use chrono::Utc;
use chrono_tz::Tz;
use std::cmp::Ord;
use std::cmp::Ordering;
use teloxide::types::ChatId;
use teloxide::types::UserId;
use teloxide::utils::markdown::{bold, escape};

pub trait GenericReminder {
    fn get_time(&self) -> NaiveDateTime;
    fn get_id(&self) -> Option<i64>;
    fn get_type(&self) -> &'static str;
    fn to_string(&self, user_timezone: Tz) -> String;
    fn to_string_with_mention(
        &self,
        user_timezone: Tz,
        user_id: i64,
    ) -> String {
        format!(
            "[🔔](tg://user?id={})\n{}",
            user_id,
            self.to_string(user_timezone),
        )
    }
    fn to_unescaped_string(&self, user_timezone: Tz) -> String;
    fn serialize_time_unescaped(&self, user_timezone: Tz) -> String {
        let time = user_timezone.from_utc_datetime(&self.get_time());
        let now = Utc::now().with_timezone(&user_timezone);
        let mut s = String::new();
        if time.date_naive() != now.date_naive() {
            s += &format!("{:02}.{:02} ", time.day(), time.month());
        }
        s + &format!("{:02}:{:02}", time.hour(), time.minute())
    }
    fn serialize_time(&self, user_timezone: Tz) -> String {
        escape(&self.serialize_time_unescaped(user_timezone))
    }
    fn user_id(&self) -> Option<UserId>;
    fn chat_id(&self) -> ChatId;
    fn is_group(&self) -> bool {
        let chat_id = self.chat_id();
        chat_id.is_group() || chat_id.is_channel_or_supergroup()
    }
}

impl GenericReminder for reminder::ActiveModel {
    fn get_time(&self) -> NaiveDateTime {
        self.time.clone().unwrap()
    }

    fn get_id(&self) -> Option<i64> {
        self.id.clone().take()
    }

    fn get_type(&self) -> &'static str {
        "rem"
    }

    fn to_unescaped_string(&self, user_timezone: Tz) -> String {
        format!(
            "{} <{}>",
            self.serialize_time_unescaped(user_timezone),
            self.desc.clone().unwrap(),
        )
    }

    fn to_string(&self, user_timezone: Tz) -> String {
        format!(
            r"{} <{}\>",
            self.serialize_time(user_timezone),
            bold(&escape(&self.desc.clone().unwrap())),
        )
    }

    fn user_id(&self) -> Option<UserId> {
        self.user_id.clone().unwrap().map(|id| UserId(id as u64))
    }

    fn chat_id(&self) -> ChatId {
        ChatId(self.chat_id.clone().unwrap())
    }
}

impl GenericReminder for cron_reminder::ActiveModel {
    fn get_time(&self) -> NaiveDateTime {
        self.time.clone().unwrap()
    }

    fn get_id(&self) -> Option<i64> {
        self.id.clone().take()
    }

    fn get_type(&self) -> &'static str {
        "cron_rem"
    }

    fn to_unescaped_string(&self, user_timezone: Tz) -> String {
        let s = format!(
            "{} <{}> [{}]",
            self.serialize_time_unescaped(user_timezone),
            self.desc.clone().unwrap(),
            self.cron_expr.clone().unwrap()
        );
        if self.paused.clone().unwrap() {
            format!("⏸ {}", s)
        } else {
            s
        }
    }

    fn to_string(&self, user_timezone: Tz) -> String {
        format!(
            r"{} <{}\> \[{}\]",
            self.serialize_time(user_timezone),
            bold(&escape(&self.desc.clone().unwrap())),
            escape(&self.cron_expr.clone().unwrap())
        )
    }

    fn user_id(&self) -> Option<UserId> {
        self.user_id.clone().unwrap().map(|id| UserId(id as u64))
    }

    fn chat_id(&self) -> ChatId {
        ChatId(self.chat_id.clone().unwrap())
    }
}

impl Ord for dyn GenericReminder {
    fn cmp(&self, other: &Self) -> Ordering {
        self.get_time().cmp(&other.get_time())
    }
}

impl PartialOrd for dyn GenericReminder {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for dyn GenericReminder {
    fn eq(&self, other: &Self) -> bool {
        self.get_time() == other.get_time()
    }
}

impl Eq for dyn GenericReminder {}
