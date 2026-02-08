use crate::serializers::Pattern;

use crate::entity::reminder;
use chrono::prelude::*;
#[cfg(not(test))]
use chrono::Utc;
use chrono_tz::Tz;
use sea_orm::ActiveValue::{NotSet, Set};
use serde_json::to_string;

#[cfg(not(test))]
pub(crate) fn now_time() -> NaiveDateTime {
    Utc::now().naive_utc()
}

fn nag_interval_to_secs(interval: &remindee_parser::Interval) -> Option<i64> {
    if interval.years != 0 || interval.months != 0 {
        return None;
    }

    let mut total_secs = 0i64;
    for (value, multiplier) in [
        (interval.weeks as i64, 7 * 24 * 60 * 60),
        (interval.days as i64, 24 * 60 * 60),
        (interval.hours as i64, 60 * 60),
        (interval.minutes as i64, 60),
        (interval.seconds as i64, 1),
    ] {
        total_secs = total_secs.checked_add(value.checked_mul(multiplier)?)?;
    }

    if total_secs > 0 {
        Some(total_secs)
    } else {
        None
    }
}

pub(crate) async fn parse_reminder(
    s: &str,
    chat_id: i64,
    user_id: u64,
    rec_id: String,
    user_timezone: Tz,
) -> Option<reminder::ActiveModel> {
    let rem = remindee_parser::parse_reminder(s)?;
    let remindee_parser::Reminder {
        description,
        pattern,
        nag_interval,
    } = rem;

    let description = description.map(|x| x.0).unwrap_or("".to_owned());
    let nag_interval_sec = match nag_interval.as_ref() {
        Some(interval) => Some(nag_interval_to_secs(interval)?),
        None => None,
    };
    let mut pattern = Pattern::from_with_tz(pattern?, user_timezone).ok()?;
    let time = pattern.next(now_time())?;
    // Convert to UTC
    Some(reminder::ActiveModel {
        id: NotSet,
        chat_id: Set(chat_id),
        user_id: Set(Some(user_id as i64)),
        time: Set(time),
        desc: Set(description),
        paused: Set(false),
        nag_interval_sec: Set(nag_interval_sec),
        pattern: Set(to_string(&pattern).ok()),
        rec_id: Set(rec_id),
    })
}

#[cfg(test)]
pub(crate) fn now_time() -> NaiveDateTime {
    DateTime::from_timestamp(*test::TEST_TIMESTAMP.read().unwrap(), 0)
        .unwrap()
        .naive_utc()
}

#[cfg(test)]
pub(crate) mod test {
    use super::*;
    use crate::serializers::Pattern;
    use serde_json::from_str;
    use serial_test::serial;
    use test_case::test_case;
    extern crate strfmt;
    use std::{collections::HashMap, sync::RwLock};
    use strfmt::strfmt;

    lazy_static! {
        pub(crate) static ref TEST_TZ: Tz =
            "Europe/Moscow".parse::<Tz>().unwrap();
        pub(crate) static ref TEST_TIME: DateTime<Tz> =
            TEST_TZ.with_ymd_and_hms(2007, 2, 2, 12, 30, 30).unwrap();
    }

    pub(crate) static TEST_TIMESTAMP: RwLock<i64> = RwLock::new(0);
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
    #[serial]
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
        *TEST_TIMESTAMP.write().unwrap() = TEST_TIME.timestamp();
        let result = parse_reminder(
            &strfmt(fmt_str, &vars).unwrap(),
            0,
            0,
            "0:0".to_owned(),
            *TEST_TZ,
        )
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

    #[tokio::test]
    #[serial]
    async fn test_parse_cron_reminder_valid() {
        *TEST_TIMESTAMP.write().unwrap() = TEST_TIME.timestamp();
        let result = parse_reminder(
            "cron: */5 * * * * cron test",
            0,
            0,
            "0:0".to_owned(),
            *TEST_TZ,
        )
        .await;
        let reminder = result.expect("cron reminder should parse");
        assert_eq!(reminder.desc.unwrap(), "cron test".to_owned());
        let pattern_json = reminder.pattern.unwrap().unwrap();
        let pattern: Pattern = from_str(&pattern_json).unwrap();
        assert!(matches!(pattern, Pattern::Cron(_)));
    }

    #[tokio::test]
    #[serial]
    async fn test_parse_cron_reminder_invalid() {
        *TEST_TIMESTAMP.write().unwrap() = TEST_TIME.timestamp();
        let result = parse_reminder(
            "cron: */5 * * * invalid",
            0,
            0,
            "0:0".to_owned(),
            *TEST_TZ,
        )
        .await;
        assert!(result.is_none());
    }

    #[tokio::test]
    #[serial]
    async fn test_parse_reminder_with_recurrence_nag_interval() {
        *TEST_TIMESTAMP.write().unwrap() = TEST_TIME.timestamp();
        let result = parse_reminder(
            "12:40!10m take pill",
            0,
            0,
            "0:0".to_owned(),
            *TEST_TZ,
        )
        .await
        .expect("reminder should parse");
        assert_eq!(result.nag_interval_sec.unwrap(), Some(600));
    }

    #[tokio::test]
    #[serial]
    async fn test_parse_reminder_with_countdown_nag_interval() {
        *TEST_TIMESTAMP.write().unwrap() = TEST_TIME.timestamp();
        let result = parse_reminder(
            "30m!5m turn off stove",
            0,
            0,
            "0:0".to_owned(),
            *TEST_TZ,
        )
        .await
        .expect("countdown reminder should parse");
        assert_eq!(result.nag_interval_sec.unwrap(), Some(300));
    }

    #[tokio::test]
    #[serial]
    async fn test_parse_reminder_with_invalid_nag_interval() {
        *TEST_TIMESTAMP.write().unwrap() = TEST_TIME.timestamp();
        let result = parse_reminder(
            "12:40!1mo take pill",
            0,
            0,
            "0:0".to_owned(),
            *TEST_TZ,
        )
        .await;
        assert!(result.is_none());
    }

    #[tokio::test]
    #[serial]
    async fn test_parse_reminder_with_malformed_nag_suffix() {
        *TEST_TIMESTAMP.write().unwrap() = TEST_TIME.timestamp();
        let result = parse_reminder(
            "12:40!10mTake pill",
            0,
            0,
            "0:0".to_owned(),
            *TEST_TZ,
        )
        .await;
        assert!(result.is_none());
    }
}
