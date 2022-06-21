use crate::date;
use crate::db;

use chrono::offset::TimeZone;
use chrono::prelude::*;
use chrono::{Duration, Utc};
use chrono_tz::Tz;
use cron_parser::parse as parse_cron;
use regex::Regex;

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
    user_id: i64,
    user_timezone: Tz,
) -> Option<db::ReminderStruct> {
    lazy_static! {
        static ref RE: Regex = Regex::new(&format!(
            concat!(
                r"^\s*((?P<{day}>\d{{1,2}})(\.(?P<{month}>\d{{1,2}}))?(\.(?P<{year}>\d{{4}}))?\s+)?",
                r"(?P<{hour}>\d{{1,2}})(:(?P<{minute}>\d{{1,2}})(:(?P<{second}>\d{{2}}))?)?(\s|$)\s*",
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
        let now = user_timezone.from_utc_datetime(&now_time());
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
            .and_hms(hour, minute, second);

        if time < now {
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
                if time + duration >= now {
                    time = time + duration;
                    break;
                }
            }
        }
        Some(db::ReminderStruct {
            id: 0,
            user_id,
            time: time.with_timezone(&Utc).naive_utc(),
            desc: caps[ReminderRegexFields::DESCRIPTION].to_string(),
            sent: false,
            edit: false,
        })
    })
}

pub async fn parse_cron_reminder(
    text: &str,
    user_id: i64,
    user_timezone: Tz,
) -> Option<db::CronReminderStruct> {
    let cron_fields: Vec<&str> = text.split_whitespace().take(5).collect();
    if cron_fields.len() < 5 {
        None
    } else {
        let cron_expr = cron_fields.join(" ");
        parse_cron(&cron_expr, &Utc::now().with_timezone(&user_timezone))
            .map(|time| db::CronReminderStruct {
                id: 0,
                user_id,
                cron_expr: cron_expr.clone(),
                time: time.with_timezone(&Utc).naive_utc(),
                desc: text
                    .strip_prefix(&(cron_expr.to_owned()))
                    .unwrap_or("")
                    .trim()
                    .to_owned(),
                sent: false,
                edit: false,
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

    lazy_static! {
        static ref TEST_TZ: Tz = "Europe/Moscow".parse::<Tz>().unwrap();
    }

    pub static mut TEST_TIMESTAMP: i64 = 0;
    const TEST_DESCRIPTION: &str = "reminder description";

    async fn test_parse_reminder(
        text: &str,
        mock_time: NaiveDateTime,
    ) -> Option<(DateTime<Tz>, String)> {
        unsafe {
            TEST_TIMESTAMP = mock_time.timestamp();
        }
        parse_reminder(text, 0, *TEST_TZ)
            .await
            .and_then(|reminder| {
                Some((TEST_TZ.from_utc_datetime(&reminder.time), reminder.desc))
            })
    }

    #[tokio::test]
    async fn test_parse_ordinary_reminder() {
        let (year, month, day, hour, minute, second) =
            (2007, 1, 1, 22, 0, 0);
        let expected_time = TEST_TZ.ymd(year, month, day).and_hms(hour, minute, second);
        assert_eq!(
            test_parse_reminder(
                format!("{}:{} {}", hour, minute, TEST_DESCRIPTION).as_str(),
                NaiveDate::from_ymd(year, month, day).and_hms(0, 0, 0),
            ).await,
            Some((expected_time, TEST_DESCRIPTION.to_owned()))
        );
    }
}
