use crate::date;

use crate::entity::{cron_reminder, reminder};
use chrono::offset::TimeZone;
use chrono::prelude::*;
use chrono::{Duration, Utc};
use chrono_tz::Tz;
use cron_parser::parse as parse_cron;
use regex::Regex;
use sea_orm::ActiveValue::{NotSet, Set};

#[non_exhaustive]
struct ReminderRegexFields;

impl ReminderRegexFields {
    const DAY: &'static str = "day";
    const MONTH: &'static str = "month";
    const YEAR: &'static str = "year";
    const HOUR: &'static str = "hour";
    const MINUTE: &'static str = "minute";
    const SECOND: &'static str = "second";
    const DESCRIPTION: &'static str = "description";
}

#[cfg(not(test))]
fn now_time() -> NaiveDateTime {
    Utc::now().naive_utc()
}

pub async fn parse_reminder(
    s: &str,
    chat_id: i64,
    user_id: u64,
    user_timezone: Tz,
) -> Option<reminder::ActiveModel> {
    lazy_static! {
        static ref RE: Regex = Regex::new(&format!(
            concat!(
                r"^\s*((?P<{day}>\d{{1,2}})(\.(?P<{month}>\d{{1,2}}))?(\.(?P<{year}>\d{{4}}))?\s+)?",
                r"(?P<{hour}>\d{{1,2}})(:(?P<{minute}>\d{{1,2}})(:(?P<{second}>\d{{1,2}}))?)?(\s|$)\s*",
                r"(?P<{description}>(?s:.)*?)\s*$"
            ),
            day = ReminderRegexFields::DAY,
            month = ReminderRegexFields::MONTH,
            year = ReminderRegexFields::YEAR,
            hour = ReminderRegexFields::HOUR,
            minute = ReminderRegexFields::MINUTE,
            second = ReminderRegexFields::SECOND,
            description = ReminderRegexFields::DESCRIPTION
        ))
        .unwrap();
    }

    RE.captures(s).and_then(|caps| {
        let now = user_timezone.from_utc_datetime(&now_time()).naive_local();
        let get_field_by_name_or = |name, default| {
            caps.name(name)
                .and_then(|x| x.as_str().parse().ok())
                .unwrap_or(default)
        };
        let day = get_field_by_name_or(ReminderRegexFields::DAY, now.day());
        let month =
            get_field_by_name_or(ReminderRegexFields::MONTH, now.month());
        let year =
            get_field_by_name_or(ReminderRegexFields::YEAR, now.year() as u32)
                as i32;
        let hour = get_field_by_name_or(ReminderRegexFields::HOUR, now.hour());
        let default_minute_value = match caps.name(ReminderRegexFields::HOUR) {
            Some(_) => 0,
            None => now.minute(),
        };
        let minute = get_field_by_name_or(
            ReminderRegexFields::MINUTE,
            default_minute_value,
        );
        let second = get_field_by_name_or(ReminderRegexFields::SECOND, 0);

        if !((0..24).contains(&hour)
            && (0..60).contains(&minute)
            && (0..60).contains(&second))
        {
            return None;
        }

        let mut time = now
            .date()
            .with_day(day)
            .and_then(|x| x.with_month(month))
            .and_then(|x| x.with_year(year))
            .unwrap_or_else(|| now.date())
            .and_hms_opt(hour, minute, second)?;

        if time <= now {
            let specified_day = caps.name(ReminderRegexFields::DAY).is_some();
            let specified_month =
                caps.name(ReminderRegexFields::MONTH).is_some();
            let durations = if !specified_day || specified_month {
                [
                    1,
                    date::days_in_month(month, year),
                    date::days_in_year(year),
                ]
                .to_vec()
            } else {
                [date::days_in_month(month, year), date::days_in_year(year)]
                    .to_vec()
            };
            for duration in durations.iter().map(|&x| Duration::days(x)) {
                if time.date().and_hms_opt(0, 0, 0)? + duration > now {
                    time += duration;
                    break;
                }
            }
        }
        // Convert to UTC
        Some(reminder::ActiveModel {
            id: NotSet,
            chat_id: Set(chat_id),
            user_id: Set(Some(user_id as i64)),
            time: Set(user_timezone
                .from_local_datetime(&time)
                .single()?
                .naive_utc()),
            desc: Set(caps[ReminderRegexFields::DESCRIPTION].to_string()),
            sent: Set(false),
            edit: Set(false),
        })
    })
}

pub async fn parse_cron_reminder(
    text: &str,
    chat_id: i64,
    user_id: u64,
    user_timezone: Tz,
) -> Option<cron_reminder::ActiveModel> {
    let cron_fields: Vec<&str> = text.split_whitespace().take(5).collect();
    if cron_fields.len() < 5 {
        None
    } else {
        let cron_expr = cron_fields.join(" ");
        parse_cron(&cron_expr, &Utc::now().with_timezone(&user_timezone))
            .map(|time| cron_reminder::ActiveModel {
                id: NotSet,
                chat_id: Set(chat_id),
                user_id: Set(Some(user_id as i64)),
                cron_expr: Set(cron_expr.clone()),
                time: Set(time.with_timezone(&Utc).naive_utc()),
                desc: Set(text
                    .strip_prefix(&(cron_expr.to_owned()))
                    .unwrap_or("")
                    .trim()
                    .to_owned()),
                sent: Set(false),
                edit: Set(false),
            })
            .ok()
    }
}

#[cfg(test)]
fn now_time() -> NaiveDateTime {
    unsafe { NaiveDateTime::from_timestamp(test::TEST_TIMESTAMP, 0) }
}
#[cfg(test)]
mod test {
    use super::*;
    use test_case::test_case;
    extern crate strfmt;
    use std::collections::HashMap;
    use strfmt::strfmt;

    lazy_static! {
        static ref TEST_TZ: Tz = "Europe/Moscow".parse::<Tz>().unwrap();
        static ref TEST_TIME: DateTime<Tz> =
            TEST_TZ.ymd(2007, 2, 2).and_hms(12, 30, 30);
    }

    pub static mut TEST_TIMESTAMP: i64 = 0;
    const TEST_DESCRIPTION: &str = "reminder description";

    #[derive(Debug, PartialEq)]
    struct Time(i32, u32, u32, u32, u32, u32);

    #[test_case("{day}.{month}.{year} {hour}:{minute}:{second} {desc}", Time(2008, 2, 2, 12, 31, 1) => Some(Time(2008, 2, 2, 12, 31, 1)) ; "ymd hms" )]
    #[test_case("{day}.{month}.{year} {hour}:{minute} {desc}", Time(2007, 2, 2, 12, 31, 1) => Some(Time(2007, 2, 2, 12, 31, 0)) ; "ymd hm" )]
    #[test_case("{day}.{month}.{year} {hour} {desc}", Time(2007, 2, 2, 13, 0, 0) => Some(Time(2007, 2, 2, 13, 0, 0)) ; "ymd h" )]
    #[test_case("{day}.{month}.{year} {desc}", Time(2007, 2, 2, 0, 0, 0) => None ; "ymd non-parsable" )]
    #[test_case("{hour}:{minute} {desc}", Time(2007, 2, 2, 12, 40, 0) => Some(Time(2007, 2, 2, 12, 40, 0)) ; "hm" )]
    #[test_case("{day}.{month} {hour} {desc}", Time(2007, 2, 2, 13, 31, 1) => Some(Time(2007, 2, 2, 13, 0, 0)) ; "md h" )]
    #[test_case("{day} {hour} {desc}", Time(2007, 2, 2, 13, 31, 1) => Some(Time(2007, 2, 2, 13, 0, 0)) ; "d h" )]
    #[test_case("{hour}:{minute} {desc}", Time(2007, 2, 2, 11, 0, 0) => Some(Time(2007, 2, 3, 11, 0, 0)) ; "hour before" )]
    #[test_case("{hour}:{minute} {desc}", Time(2007, 2, 2, 12, 29, 0) => Some(Time(2007, 2, 3, 12, 29, 0)) ; "minute before" )]
    #[test_case("{day} {hour} {desc}", Time(2007, 2, 1, 13, 0, 0) => Some(Time(2007, 3, 1, 13, 0, 0)) ; "day before" )]
    #[test_case("02.01 13:00 {desc}", Time(2007, 1, 2, 13, 0, 0) => Some(Time(2008, 1, 2, 13, 0, 0)) ; "month before" )]
    #[test_case("{hour}:{minute}{desc}", Time(2007, 2, 2, 12, 30, 0) => None ; "non-parsable" )]
    #[tokio::test]
    async fn test_parse_reminder(fmt_str: &str, time: Time) -> Option<Time> {
        let (year, month, day, hour, minute, second) =
            (time.0, time.1, time.2, time.3, time.4, time.5);
        let vars = HashMap::from([
            ("year".to_owned(), year.to_string()),
            ("month".to_owned(), month.to_string()),
            ("day".to_owned(), day.to_string()),
            ("hour".to_owned(), hour.to_string()),
            ("minute".to_owned(), minute.to_string()),
            ("second".to_owned(), second.to_string()),
            ("desc".to_owned(), TEST_DESCRIPTION.to_owned()),
        ]);
        unsafe {
            TEST_TIMESTAMP = TEST_TIME.timestamp();
        }
        dbg!("{}", strfmt(fmt_str, &vars).unwrap());
        let result =
            parse_reminder(&strfmt(fmt_str, &vars).unwrap(), 0, 0u64, *TEST_TZ)
                .await
                .map(|reminder| {
                    (
                        TEST_TZ.from_utc_datetime(&reminder.time.unwrap()),
                        reminder.desc.unwrap(),
                    )
                });
        match result {
            Some((time, desc)) => {
                assert_eq!(desc, TEST_DESCRIPTION.to_owned());
                Some(Time(
                    time.year(),
                    time.month(),
                    time.day(),
                    time.hour(),
                    time.minute(),
                    time.second(),
                ))
            }
            None => None,
        }
    }
}
