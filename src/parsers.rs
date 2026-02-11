use crate::serializers::Pattern;

use crate::date;
use crate::entity::reminder;
use crate::serializers::{DateDivisor, DatePattern};
use chrono::prelude::*;
#[cfg(not(test))]
use chrono::Utc;
use chrono_tz::Tz;
use remindee_parser::{ReminderPattern, TimePattern};
use sea_orm::ActiveValue::{NotSet, Set};
use serde_json::to_string;

#[cfg(not(test))]
pub(crate) fn now_time() -> NaiveDateTime {
    Utc::now().naive_utc()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ParseError {
    InvalidFormat,
    DateInPast,
    TimeRangeEndBeforeStart,
    DateRangeEndBeforeStart,
    TimeIntervalTooLargeForRange,
    DateIntervalTooLargeForRange,
    NagIntervalUnsupportedUnit,
    CronExpressionInvalid,
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

fn starts_with_past_full_date(s: &str, user_timezone: Tz) -> bool {
    let lower_bound = user_timezone
        .from_utc_datetime(&now_time())
        .naive_local()
        .date();
    s.split_whitespace()
        .next()
        .and_then(|token| NaiveDate::parse_from_str(token, "%d.%m.%Y").ok())
        .map(|date| date < lower_bound)
        .unwrap_or(false)
}

fn time_to_seconds(time: &remindee_parser::Time) -> i64 {
    (time.hour as i64) * 60 * 60
        + (time.minute as i64) * 60
        + (time.second as i64)
}

fn time_interval_to_seconds(interval: &remindee_parser::TimeInterval) -> i64 {
    (interval.hours as i64) * 60 * 60
        + (interval.minutes as i64) * 60
        + (interval.seconds as i64)
}

fn has_time_interval_too_large_for_range(pattern: &ReminderPattern) -> bool {
    let ReminderPattern::Recurrence(recurrence) = pattern else {
        return false;
    };

    recurrence.time_patterns.iter().any(|time_pattern| {
        let TimePattern::Range(range) = time_pattern else {
            return false;
        };
        match (&range.from, &range.until) {
            (Some(from), Some(until)) => {
                let span = time_to_seconds(until) - time_to_seconds(from);
                let interval = time_interval_to_seconds(&range.interval);
                interval > span
            }
            _ => false,
        }
    })
}

fn has_time_range_end_before_start(pattern: &ReminderPattern) -> bool {
    let ReminderPattern::Recurrence(recurrence) = pattern else {
        return false;
    };

    recurrence.time_patterns.iter().any(|time_pattern| {
        let TimePattern::Range(range) = time_pattern else {
            return false;
        };
        match (&range.from, &range.until) {
            (Some(from), Some(until)) => {
                time_to_seconds(until) <= time_to_seconds(from)
            }
            _ => false,
        }
    })
}

fn holey_date_to_naive(
    holey_date: &remindee_parser::HoleyDate,
) -> Option<NaiveDate> {
    NaiveDate::from_ymd_opt(
        holey_date.year?,
        holey_date.month?,
        holey_date.day?,
    )
}

fn holey_date_to_naive_with_defaults(
    holey_date: &remindee_parser::HoleyDate,
    default_year: Option<i32>,
    default_month: Option<u32>,
    default_day: Option<u32>,
) -> Option<NaiveDate> {
    let year = holey_date.year.or(default_year)?;
    let month = holey_date.month.or(default_month)?;
    let day = date::normalise_day(year, month, holey_date.day.or(default_day)?);
    NaiveDate::from_ymd_opt(year, month, day)
}

fn has_date_range_end_before_start(pattern: &ReminderPattern) -> bool {
    let ReminderPattern::Recurrence(recurrence) = pattern else {
        return false;
    };

    recurrence.dates_patterns.iter().any(|date_pattern| {
        let remindee_parser::DatePattern::Range(range) = date_pattern else {
            return false;
        };
        let Some(until) = &range.until else {
            return false;
        };

        match (
            holey_date_to_naive_with_defaults(
                &range.from,
                until.year,
                until.month,
                until.day,
            ),
            holey_date_to_naive_with_defaults(
                until,
                range.from.year,
                range.from.month,
                range.from.day,
            ),
        ) {
            (Some(from), Some(until)) => until < from,
            _ => false,
        }
    })
}

fn has_explicit_past_date(
    pattern: &ReminderPattern,
    user_timezone: Tz,
) -> bool {
    let ReminderPattern::Recurrence(recurrence) = pattern else {
        return false;
    };
    let lower_bound = user_timezone
        .from_utc_datetime(&now_time())
        .naive_local()
        .date();

    recurrence
        .dates_patterns
        .iter()
        .any(|date_pattern| match date_pattern {
            remindee_parser::DatePattern::Point(holey_date) => {
                holey_date_to_naive(holey_date)
                    .map(|date| date < lower_bound)
                    .unwrap_or(false)
            }
            remindee_parser::DatePattern::Range(range) => range
                .until
                .as_ref()
                .and_then(holey_date_to_naive)
                .map(|until| until < lower_bound)
                .unwrap_or(false),
        })
}

fn has_date_interval_too_large_for_range(pattern: &Pattern) -> bool {
    let Pattern::Recurrence(recurrence) = pattern else {
        return false;
    };

    recurrence.dates_patterns.iter().any(|date_pattern| {
        let DatePattern::Range(range) = date_pattern else {
            return false;
        };
        let (Some(until), DateDivisor::Interval(interval)) =
            (&range.until, &range.date_divisor)
        else {
            return false;
        };

        date::add_date_interval(range.from, interval)
            .map(|next_date| next_date > *until)
            .unwrap_or(false)
    })
}

fn nag_interval_to_secs_result(
    interval: &remindee_parser::Interval,
) -> Result<i64, ParseError> {
    if interval.years != 0 || interval.months != 0 {
        return Err(ParseError::NagIntervalUnsupportedUnit);
    }
    nag_interval_to_secs(interval).ok_or(ParseError::InvalidFormat)
}

pub(crate) async fn parse_reminder_detailed(
    s: &str,
    chat_id: i64,
    user_id: u64,
    rec_id: String,
    user_timezone: Tz,
) -> Result<reminder::ActiveModel, ParseError> {
    let rem = remindee_parser::parse_reminder(s).ok_or_else(|| {
        if starts_with_past_full_date(s, user_timezone) {
            ParseError::DateInPast
        } else {
            ParseError::InvalidFormat
        }
    })?;
    let remindee_parser::Reminder {
        description,
        pattern,
        nag_interval,
    } = rem;
    let pattern = pattern.ok_or(ParseError::InvalidFormat)?;

    if has_time_range_end_before_start(&pattern) {
        return Err(ParseError::TimeRangeEndBeforeStart);
    }

    if has_date_range_end_before_start(&pattern) {
        return Err(ParseError::DateRangeEndBeforeStart);
    }

    if has_time_interval_too_large_for_range(&pattern) {
        return Err(ParseError::TimeIntervalTooLargeForRange);
    }

    let description = description.map(|x| x.0).unwrap_or("".to_owned());
    let nag_interval_sec = match nag_interval.as_ref() {
        Some(interval) => Some(nag_interval_to_secs_result(interval)?),
        None => None,
    };
    let is_cron_pattern = matches!(&pattern, ReminderPattern::Cron(_));
    let has_past_date = has_explicit_past_date(&pattern, user_timezone);
    let mut pattern =
        Pattern::from_with_tz(pattern, user_timezone).map_err(|_| {
            if is_cron_pattern {
                ParseError::CronExpressionInvalid
            } else if has_past_date {
                ParseError::DateInPast
            } else {
                ParseError::InvalidFormat
            }
        })?;

    if has_date_interval_too_large_for_range(&pattern) {
        return Err(ParseError::DateIntervalTooLargeForRange);
    }

    let time = pattern.next(now_time()).ok_or(ParseError::InvalidFormat)?;
    Ok(reminder::ActiveModel {
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
pub(crate) async fn parse_reminder(
    s: &str,
    chat_id: i64,
    user_id: u64,
    rec_id: String,
    user_timezone: Tz,
) -> Option<reminder::ActiveModel> {
    parse_reminder_detailed(s, chat_id, user_id, rec_id, user_timezone)
        .await
        .ok()
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

    #[tokio::test]
    #[serial]
    async fn test_parse_reminder_reports_time_interval_too_large_for_range() {
        *TEST_TIMESTAMP.write().unwrap() = TEST_TIME.timestamp();
        let result = parse_reminder_detailed(
            "14-15/2h bad range",
            0,
            0,
            "0:0".to_owned(),
            *TEST_TZ,
        )
        .await;
        assert!(matches!(
            result,
            Err(ParseError::TimeIntervalTooLargeForRange)
        ));
    }

    #[tokio::test]
    #[serial]
    async fn test_parse_reminder_reports_time_range_end_before_start() {
        *TEST_TIMESTAMP.write().unwrap() = TEST_TIME.timestamp();
        let result = parse_reminder_detailed(
            "18-10/1h bad range",
            0,
            0,
            "0:0".to_owned(),
            *TEST_TZ,
        )
        .await;
        assert!(matches!(result, Err(ParseError::TimeRangeEndBeforeStart)));
    }

    #[tokio::test]
    #[serial]
    async fn test_parse_reminder_reports_date_in_past() {
        *TEST_TIMESTAMP.write().unwrap() = TEST_TIME.timestamp();
        let result = parse_reminder_detailed(
            "10.10.2000 ff",
            0,
            0,
            "0:0".to_owned(),
            *TEST_TZ,
        )
        .await;
        assert!(matches!(result, Err(ParseError::DateInPast)));
    }

    #[tokio::test]
    #[serial]
    async fn test_parse_reminder_reports_date_interval_too_large_for_range() {
        *TEST_TIMESTAMP.write().unwrap() = TEST_TIME.timestamp();
        let result = parse_reminder_detailed(
            "11-12/2d 10:00 bad range",
            0,
            0,
            "0:0".to_owned(),
            *TEST_TZ,
        )
        .await;
        assert!(matches!(
            result,
            Err(ParseError::DateIntervalTooLargeForRange)
        ));
    }

    #[tokio::test]
    #[serial]
    async fn test_parse_reminder_reports_date_range_end_before_start() {
        *TEST_TIMESTAMP.write().unwrap() = TEST_TIME.timestamp();
        let result = parse_reminder_detailed(
            "12.02.2026-11.02.2026/1d 10:00 bad range",
            0,
            0,
            "0:0".to_owned(),
            *TEST_TZ,
        )
        .await;
        assert!(matches!(result, Err(ParseError::DateRangeEndBeforeStart)));
    }

    #[tokio::test]
    #[serial]
    async fn test_parse_reminder_reports_date_range_end_before_start_with_mixed_year(
    ) {
        *TEST_TIMESTAMP.write().unwrap() = TEST_TIME.timestamp();
        let result = parse_reminder_detailed(
            "11.02-10.02.2026/1d 10:00 bad range",
            0,
            0,
            "0:0".to_owned(),
            *TEST_TZ,
        )
        .await;
        assert!(matches!(result, Err(ParseError::DateRangeEndBeforeStart)));
    }

    #[tokio::test]
    #[serial]
    async fn test_parse_reminder_reports_nag_interval_unsupported_unit() {
        *TEST_TIMESTAMP.write().unwrap() = TEST_TIME.timestamp();
        let result = parse_reminder_detailed(
            "12:40!1mo take pill",
            0,
            0,
            "0:0".to_owned(),
            *TEST_TZ,
        )
        .await;
        assert!(matches!(
            result,
            Err(ParseError::NagIntervalUnsupportedUnit)
        ));
    }

    #[tokio::test]
    #[serial]
    async fn test_parse_reminder_reports_invalid_cron_expression() {
        *TEST_TIMESTAMP.write().unwrap() = TEST_TIME.timestamp();
        let result = parse_reminder_detailed(
            "cron: */5 * * * invalid",
            0,
            0,
            "0:0".to_owned(),
            *TEST_TZ,
        )
        .await;
        assert!(matches!(result, Err(ParseError::CronExpressionInvalid)));
    }

    #[tokio::test]
    #[serial]
    async fn test_parse_time_range_rolls_to_next_day_after_until() {
        *TEST_TIMESTAMP.write().unwrap() = TEST_TZ
            .with_ymd_and_hms(2007, 2, 2, 13, 23, 0)
            .unwrap()
            .timestamp();
        let result =
            parse_reminder("10-13/1h desc", 0, 0, "0:0".to_owned(), *TEST_TZ)
                .await
                .expect("reminder should parse");
        let dt = TEST_TZ.from_utc_datetime(&result.time.unwrap());
        assert_eq!(
            (
                dt.year(),
                dt.month(),
                dt.day(),
                dt.hour(),
                dt.minute(),
                dt.second()
            ),
            (2007, 2, 3, 10, 0, 0),
        );
    }
}
