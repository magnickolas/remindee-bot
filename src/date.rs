use std::cmp::min;

use crate::serializers::{DateInterval, Interval};
use chrono::{Datelike, NaiveDate, NaiveDateTime};
use nonempty::NonEmpty;

fn is_leap_year(year: i32) -> bool {
    year % 4 == 0 && (year % 400 == 0 || year % 100 != 0)
}

pub fn days_in_month(month: u32, year: i32) -> u32 {
    match month {
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => 31,
    }
}

pub fn days_in_year(year: i32) -> u32 {
    if is_leap_year(year) {
        366
    } else {
        365
    }
}

fn add_months(date: NaiveDateTime, months: u32) -> NaiveDateTime {
    let total_months = (date.month() - 1) + months; // 1-indexed => 0-indexed
    let year = date.year() + total_months as i32 / 12;
    let month = total_months % 12 + 1; // 0-indexed => 1-indexed
    let day = min(date.day(), days_in_month(month, year));
    NaiveDate::from_ymd_opt(year, month, day)
        .unwrap()
        .and_time(date.time())
}

pub fn add_interval(time: NaiveDateTime, interval: &Interval) -> NaiveDateTime {
    add_months(time, interval.months + interval.years as u32 * 12)
        + chrono::Duration::weeks(interval.weeks as i64)
        + chrono::Duration::days(interval.days as i64)
        + chrono::Duration::hours(interval.hours as i64)
        + chrono::Duration::minutes(interval.minutes as i64)
        + chrono::Duration::seconds(interval.seconds as i64)
}

pub fn add_date_interval(
    date: NaiveDate,
    interval: &DateInterval,
) -> NaiveDate {
    add_interval(
        date.and_hms_opt(0, 0, 0).unwrap(),
        &Interval {
            years: interval.years,
            months: interval.months,
            weeks: interval.weeks,
            days: interval.days,
            hours: 0,
            minutes: 0,
            seconds: 0,
        },
    )
    .date()
}

pub fn find_nearest_weekday(
    mut date: NaiveDate,
    weekdays: NonEmpty<u32>,
) -> NaiveDate {
    while !weekdays.contains(&date.weekday().num_days_from_monday()) {
        date += chrono::Duration::days(1);
    }
    date
}

#[cfg(test)]
mod test {
    use super::*;
    use chrono::{NaiveDateTime, NaiveTime, Timelike};
    use test_case::test_case;

    #[test_case( 1, 2023 => 31 ;       "january")]
    #[test_case( 2, 2023 => 28 ;      "february")]
    #[test_case( 3, 2023 => 31 ;         "march")]
    #[test_case( 4, 2023 => 30 ;         "april")]
    #[test_case( 5, 2023 => 31 ;           "may")]
    #[test_case( 6, 2023 => 30 ;          "june")]
    #[test_case( 7, 2023 => 31 ;          "july")]
    #[test_case( 8, 2023 => 31 ;        "august")]
    #[test_case( 9, 2023 => 30 ;     "september")]
    #[test_case(10, 2023 => 31 ;       "october")]
    #[test_case(11, 2023 => 30 ;      "november")]
    #[test_case(12, 2023 => 31 ;      "december")]
    #[test_case( 1, 2024 => 31 ;  "leap january")]
    #[test_case( 2, 2024 => 29 ; "leap february")]
    #[test_case( 3, 2024 => 31 ;    "leap march")]
    #[test_case( 4, 2024 => 30 ;    "leap april")]
    #[test_case( 5, 2024 => 31 ;      "leap may")]
    #[test_case( 6, 2024 => 30 ;     "leap june")]
    #[test_case( 7, 2024 => 31 ;     "leap july")]
    #[test_case( 8, 2024 => 31 ;   "leap august")]
    #[test_case( 9, 2024 => 30 ;"leap september")]
    #[test_case(10, 2024 => 31 ;  "leap october")]
    #[test_case(11, 2024 => 30 ; "leap november")]
    #[test_case(12, 2024 => 31 ; "leap december")]
    fn test_days_in_month(month: u32, year: i32) -> u32 {
        days_in_month(month, year)
    }

    #[test_case(2019 => 365 ;              "non-divisible by 4 year isn't leap")]
    #[test_case(2100 => 365 ; "year divisible by 100 but not by 400 isn't leap")]
    #[test_case(2000 => 366 ;                   "year divisible by 400 is leap")]
    #[test_case(2020 => 366 ;      "year divisible by 4 but not by 400 is leap")]
    fn test_days_in_year(year: i32) -> u32 {
        days_in_year(year)
    }

    #[derive(Debug, PartialEq)]
    struct Time(i32, u32, u32, u32, u32, u32);

    #[test_case(Time(2023, 9, 3, 0, 0, 0),
                Interval{years: 0, months: 1 , weeks: 0, days: 0, hours: 0, minutes: 0, seconds: 0 }
                => Time(2023, 10, 3, 0, 0, 0) ;
                "just increment month")]
    #[test_case(Time(2023, 1, 31, 0, 0, 0),
                Interval{years: 0, months: 1 , weeks: 0, days: 0, hours: 0, minutes: 0, seconds: 0 }
                => Time(2023, 2, 28, 0, 0, 0) ;
                "day should be clipped")]
    #[test_case(Time(2023, 12, 31, 0, 0, 0),
                Interval{years: 0, months: 1 , weeks: 0, days: 0, hours: 0, minutes: 0, seconds: 0 }
                => Time(2024, 1, 31, 0, 0, 0) ;
                "increment year")]
    #[test_case(Time(2023, 5, 15, 0, 0, 0),
                Interval{years: 0, months: 9 , weeks: 0, days: 0, hours: 0, minutes: 0, seconds: 0 }
                => Time(2024, 2, 15, 0, 0, 0) ;
                "increment year and month")]
    #[test_case(Time(2023, 7, 7, 0, 0, 0),
                Interval{years: 0, months: 20, weeks: 0, days: 0, hours: 0, minutes: 0, seconds: 0 }
                => Time(2025, 3, 7, 0, 0, 0) ;
                "add many months")]
    #[test_case(Time(2023, 11, 23, 22, 58, 59),
                Interval{years: 1, months: 1, weeks: 1, days: 1, hours: 1, minutes: 1, seconds: 1 }
                => Time(2025, 1, 1, 0, 0, 0) ;
                "add all units")]
    fn test_add_interval(time: Time, interval: Interval) -> Time {
        let (year, month, day, hour, minute, second) =
            (time.0, time.1, time.2, time.3, time.4, time.5);
        let datetime = NaiveDateTime::new(
            NaiveDate::from_ymd_opt(year, month, day).unwrap(),
            NaiveTime::from_hms_opt(hour, minute, second).unwrap(),
        );
        let result = add_interval(datetime, &interval);
        Time(
            result.year(),
            result.month(),
            result.day(),
            result.hour(),
            result.minute(),
            result.second(),
        )
    }
}
