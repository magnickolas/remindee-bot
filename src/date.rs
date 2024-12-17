use crate::serializers::{DateInterval, Interval};
use chrono::{Datelike, NaiveDate, NaiveDateTime};
use chronoutil::{is_leap_year, shift_months, shift_years};
use nonempty::NonEmpty;

pub(crate) fn normalise_day(year: i32, month: u32, day: u32) -> u32 {
    if day <= 28 {
        day
    } else if month == 2 {
        28 + is_leap_year(year) as u32
    } else if day == 31
        && (month == 4 || month == 6 || month == 9 || month == 11)
    {
        30
    } else {
        day
    }
}

pub(crate) fn add_interval(
    time: NaiveDateTime,
    interval: &Interval,
) -> NaiveDateTime {
    shift_months(shift_years(time, interval.years), interval.months as i32)
        + chrono::Duration::weeks(interval.weeks as i64)
        + chrono::Duration::days(interval.days as i64)
        + chrono::Duration::hours(interval.hours as i64)
        + chrono::Duration::minutes(interval.minutes as i64)
        + chrono::Duration::seconds(interval.seconds as i64)
}

pub(crate) fn add_date_interval(
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

pub(crate) fn find_nearest_weekday(
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
