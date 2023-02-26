use bitmask_enum::bitmask;
use chrono::offset::TimeZone;
use chrono::prelude::*;
use chrono::Duration;
use chrono_tz::Tz;
use nonempty::NonEmpty;
use serde::{Deserialize, Serialize};

use crate::date;
use crate::grammar;
use crate::parsers::now_time;

#[derive(Debug, Serialize, Deserialize)]
pub struct Interval {
    #[serde(rename = "y")]
    pub years: i32,
    #[serde(rename = "mo")]
    pub months: u32,
    #[serde(rename = "w")]
    pub weeks: u32,
    #[serde(rename = "d")]
    pub days: u32,
    #[serde(rename = "h")]
    pub hours: u32,
    #[serde(rename = "m")]
    pub minutes: u32,
    #[serde(rename = "s")]
    pub seconds: u32,
}

#[bitmask(u8)]
#[derive(Serialize, Deserialize)]
pub enum Weekdays {
    Monday,
    Tuesday,
    Wednesday,
    Thursday,
    Friday,
    Saturday,
    Sunday,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum DateDivisor {
    Weekdays(Weekdays),
    Interval(DateInterval),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DateRange {
    pub from: NaiveDate,
    pub until: Option<NaiveDate>,
    #[serde(rename = "div")]
    pub date_divisor: DateDivisor,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum DatePattern {
    Point(NaiveDate),
    Range(DateRange),
}

struct Time;

#[derive(Debug, Serialize, Deserialize, Copy, Clone)]
pub struct TimeInterval {
    #[serde(rename = "h")]
    pub hours: u32,
    #[serde(rename = "m")]
    pub minutes: u32,
    #[serde(rename = "s")]
    pub seconds: u32,
}

#[derive(Debug, Serialize, Deserialize, Copy, Clone)]
pub struct DateInterval {
    #[serde(rename = "y")]
    pub years: i32,
    #[serde(rename = "mo")]
    pub months: u32,
    #[serde(rename = "w")]
    pub weeks: u32,
    #[serde(rename = "d")]
    pub days: u32,
}

#[derive(Debug, Serialize, Deserialize, Copy, Clone)]
pub struct TimeRange {
    pub from: Option<NaiveTime>,
    pub until: Option<NaiveTime>,
    #[serde(rename = "int")]
    pub interval: TimeInterval,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum TimePattern {
    Point(NaiveTime),
    Range(TimeRange),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Recurrence {
    #[serde(rename = "dates")]
    pub dates_patterns: Vec<DatePattern>,
    #[serde(rename = "times")]
    pub time_patterns: Vec<TimePattern>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Countdown {
    #[serde(rename = "dur")]
    pub duration: Interval,
    used: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Period {
    #[serde(rename = "dur")]
    pub duration: Interval,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Pattern {
    Recurrence(Recurrence),
    Countdown(Countdown),
    Period(Period),
}

pub fn fill_date_holes(
    holey_date: &grammar::HoleyDate,
    lower_bound: NaiveDate,
) -> Option<NaiveDate> {
    let year = holey_date.year.unwrap_or(lower_bound.year());
    let month = holey_date.month.unwrap_or(lower_bound.month());
    let day = holey_date.day.unwrap_or(lower_bound.day());
    let time = NaiveDate::from_ymd_opt(year, month, day)?;
    if time >= lower_bound {
        return Some(time);
    }
    let increments = if holey_date.day.is_none() {
        [
            1,
            date::days_in_month(time.month(), time.year()),
            date::days_in_year(time.year()),
        ]
        .map(Into::into)
        .to_vec()
    } else if holey_date.month.is_none() {
        [
            date::days_in_month(time.month(), time.year()),
            date::days_in_year(time.year()),
        ]
        .map(Into::into)
        .to_vec()
    } else {
        [date::days_in_year(time.year())].map(Into::into).to_vec()
    };

    let mut time = time;
    for increment in increments.iter().map(|&x| Duration::days(x)) {
        if time + increment > lower_bound {
            time += increment;
            break;
        }
    }
    Some(time)
}

impl From<grammar::Interval> for Interval {
    fn from(interval: grammar::Interval) -> Self {
        Self {
            years: interval.years,
            months: interval.months,
            weeks: interval.weeks,
            days: interval.days,
            hours: interval.hours,
            minutes: interval.minutes,
            seconds: interval.seconds,
        }
    }
}

impl Weekdays {
    fn from_single_weekday(weekday: grammar::Weekdays) -> Self {
        match weekday {
            grammar::Weekdays::Monday => Self::Monday,
            grammar::Weekdays::Tuesday => Self::Tuesday,
            grammar::Weekdays::Wednesday => Self::Wednesday,
            grammar::Weekdays::Thursday => Self::Thursday,
            grammar::Weekdays::Friday => Self::Friday,
            grammar::Weekdays::Saturday => Self::Saturday,
            grammar::Weekdays::Sunday => Self::Sunday,
            _ => unreachable!(),
        }
    }
}

impl From<grammar::Weekdays> for Weekdays {
    fn from(weekdays: grammar::Weekdays) -> Self {
        let mut result = Weekdays::none();
        for weekday in [
            grammar::Weekdays::Monday,
            grammar::Weekdays::Tuesday,
            grammar::Weekdays::Wednesday,
            grammar::Weekdays::Thursday,
            grammar::Weekdays::Friday,
            grammar::Weekdays::Saturday,
            grammar::Weekdays::Sunday,
        ] {
            if weekdays.contains(weekday) {
                result |= Self::from_single_weekday(weekday);
            }
        }
        result
    }
}

impl From<grammar::DateDivisor> for DateDivisor {
    fn from(date_divisor: grammar::DateDivisor) -> Self {
        match date_divisor {
            grammar::DateDivisor::Weekdays(weekdays) => {
                Self::Weekdays(weekdays.into())
            }
            grammar::DateDivisor::Interval(interval) => {
                Self::Interval(interval.into())
            }
        }
    }
}

impl Time {
    fn from(time: &grammar::Time) -> Option<NaiveTime> {
        NaiveTime::from_hms_opt(time.hour, time.minute, time.second)
    }
}

impl From<grammar::TimeInterval> for TimeInterval {
    fn from(time_interval: grammar::TimeInterval) -> Self {
        Self {
            hours: time_interval.hours,
            minutes: time_interval.minutes,
            seconds: time_interval.seconds,
        }
    }
}

impl From<TimeInterval> for Duration {
    fn from(int: TimeInterval) -> Self {
        Duration::hours(int.hours as i64)
            + Duration::minutes(int.minutes as i64)
            + Duration::seconds(int.seconds as i64)
    }
}

impl From<grammar::DateInterval> for DateInterval {
    fn from(date_interval: grammar::DateInterval) -> Self {
        Self {
            years: date_interval.years,
            months: date_interval.months,
            weeks: date_interval.weeks,
            days: date_interval.days,
        }
    }
}

impl From<grammar::TimeRange> for TimeRange {
    fn from(time_range: grammar::TimeRange) -> Self {
        let from = time_range.from.and_then(|ref time| Time::from(time));
        let until = time_range.until.and_then(|ref time| Time::from(time));
        let interval = time_range.interval.into();
        Self {
            from,
            until,
            interval,
        }
    }
}

impl TimePattern {
    fn from(time_pattern: grammar::TimePattern) -> Option<Self> {
        match time_pattern {
            grammar::TimePattern::Point(ref time) => {
                Time::from(time).map(Self::Point)
            }
            grammar::TimePattern::Range(time_range) => {
                Some(Self::Range(time_range.into()))
            }
        }
    }

    fn get_first_time(&self) -> Option<NaiveTime> {
        match self {
            &Self::Point(time) => Some(time),
            Self::Range(ref time_range) => time_range.from,
        }
    }
}

impl Recurrence {
    pub fn from_holey(
        recurrence: grammar::Recurrence,
        lower_bound: NaiveDateTime,
    ) -> Result<Self, ()> {
        let first_time = match recurrence.time_patterns.first() {
            Some(time_pattern) => match time_pattern {
                grammar::TimePattern::Point(time) => {
                    Time::from(time).ok_or(())?
                }
                grammar::TimePattern::Range(range) => range
                    .from
                    .as_ref()
                    .and_then(Time::from)
                    .unwrap_or(NaiveTime::from_hms_opt(0, 0, 0).unwrap()),
            },
            None => lower_bound.time(),
        };
        let first_date = match recurrence.dates_patterns.first() {
            grammar::DatePattern::Point(date) => date,
            grammar::DatePattern::Range(range) => &range.from,
        };
        let has_divisor = match recurrence.dates_patterns.first() {
            grammar::DatePattern::Point(_) => false,
            grammar::DatePattern::Range(_) => true,
        };
        let has_time_divisor = recurrence
            .time_patterns
            .iter()
            .filter(|x| match x {
                grammar::TimePattern::Point(_) => false,
                grammar::TimePattern::Range(_) => true,
            })
            .count()
            > 0;
        let mut init_time = fill_date_holes(first_date, lower_bound.date())
            .map(|x| x.and_time(first_time))
            .ok_or(())?;
        if init_time < lower_bound && !has_divisor && !has_time_divisor {
            if first_date.day.is_none() {
                init_time += Duration::days(1);
            } else if first_date.month.is_none() {
                init_time += Duration::days(
                    date::days_in_month(init_time.month(), init_time.year())
                        .into(),
                );
            } else {
                init_time +=
                    Duration::days(date::days_in_year(init_time.year()).into());
            }
        }
        assert!(has_divisor || has_time_divisor || init_time >= lower_bound);
        let mut cur_lower_bound = init_time.date();
        let mut dates_patterns = vec![];
        for pattern in recurrence.dates_patterns {
            match pattern {
                grammar::DatePattern::Point(holey_date) => {
                    let date = fill_date_holes(&holey_date, cur_lower_bound)
                        .ok_or(())?;
                    dates_patterns.push(DatePattern::Point(date));
                    cur_lower_bound = date;
                }
                grammar::DatePattern::Range(grammar::DateRange {
                    from,
                    until,
                    date_divisor,
                }) => {
                    let date_from =
                        fill_date_holes(&from, cur_lower_bound).ok_or(())?;
                    cur_lower_bound = date_from;
                    let date_until = until.and_then(|until| {
                        let date = fill_date_holes(&until, cur_lower_bound)?;
                        cur_lower_bound = date;
                        Some(date)
                    });
                    dates_patterns.push(DatePattern::Range(DateRange {
                        from: date_from,
                        until: date_until,
                        date_divisor: date_divisor.into(),
                    }));
                }
            }
        }
        let time_patterns = recurrence
            .time_patterns
            .into_iter()
            .map(TimePattern::from)
            .collect::<Option<Vec<_>>>()
            .ok_or(())?;
        Ok(Self {
            dates_patterns,
            time_patterns,
        })
    }

    pub fn next(&self, cur: NaiveDateTime) -> Option<NaiveDateTime> {
        let first_date = match self.dates_patterns.first() {
            Some(&DatePattern::Point(date)) => date,
            Some(DatePattern::Range(ref range)) => range.from,
            None => return None,
        };
        let first_time = match self.time_patterns.first() {
            Some(time_pattern) => match time_pattern {
                &TimePattern::Point(time) => time,
                TimePattern::Range(ref range) => range
                    .from
                    .unwrap_or(NaiveTime::from_hms_opt(0, 0, 0).unwrap()),
            },
            None => cur.time(),
        };
        let cur_date = cur.date();
        let cur_time = cur.time();
        if first_date > cur_date {
            return Some(first_date.and_time(first_time));
        }
        let next_time = self
            .time_patterns
            .iter()
            .filter(|&int| match int {
                &TimePattern::Point(time) => time > cur_time,
                TimePattern::Range(ref range) => {
                    range.until.map(|x| x > cur_time).unwrap_or(true)
                }
            })
            .flat_map(|int| match int {
                &TimePattern::Point(time) => Some(time),
                TimePattern::Range(ref range) => {
                    let from = range
                        .from
                        .unwrap_or(NaiveTime::from_hms_opt(0, 0, 0).unwrap());
                    if from > cur_time {
                        Some(from)
                    } else {
                        let next_time = from
                            + Duration::seconds(
                                ((cur_time - from).num_seconds()
                                    / Into::<Duration>::into(range.interval)
                                        .num_seconds()
                                    + 1)
                                    * Into::<Duration>::into(range.interval)
                                        .num_seconds(),
                            );
                        if next_time > cur_time
                            && range
                                .until
                                .map(|x| next_time <= x)
                                .unwrap_or(true)
                        {
                            Some(next_time)
                        } else {
                            None
                        }
                    }
                }
            })
            .min();
        if let Some(next_time) = next_time {
            return Some(cur_date.and_time(next_time));
        }
        let next_date = self
            .dates_patterns
            .iter()
            .filter(|&int| match int {
                &DatePattern::Point(date) => date > cur_date,
                DatePattern::Range(ref range) => {
                    range.until.map(|x| x > cur_date).unwrap_or(true)
                }
            })
            .flat_map(|int| match int {
                &DatePattern::Point(date) => Some(date),
                DatePattern::Range(ref range) => {
                    let from = range.from;
                    if from > cur_date {
                        Some(from)
                    } else {
                        let next_date = match range.date_divisor {
                            DateDivisor::Weekdays(weekdays) => {
                                let weekdays = (0..7)
                                    .filter(|i| weekdays.bits() & (1 << i) != 0)
                                    .collect::<Vec<_>>();
                                date::find_nearest_weekday(
                                    cur_date + Duration::days(1),
                                    NonEmpty::from_vec(weekdays).unwrap(),
                                )
                            }
                            DateDivisor::Interval(int) => {
                                let mut from = from;
                                while from <= cur_date {
                                    from = date::add_date_interval(from, &int);
                                }
                                from
                            }
                        };
                        if range.until.map(|x| next_date <= x).unwrap_or(true) {
                            Some(next_date)
                        } else {
                            None
                        }
                    }
                }
            })
            .min();

        next_date.map(|next_date| next_date.and_time(first_time))
    }
}

impl Countdown {
    pub fn next(&mut self, cur: NaiveDateTime) -> Option<NaiveDateTime> {
        if self.used {
            None
        } else {
            self.used = true;
            Some(date::add_interval(cur, &self.duration))
        }
    }
}

impl Period {
    pub fn next(&self, cur: NaiveDateTime) -> NaiveDateTime {
        date::add_interval(cur, &self.duration)
    }
}

impl From<grammar::Countdown> for Countdown {
    fn from(countdown: grammar::Countdown) -> Self {
        Self {
            duration: countdown.duration.into(),
            used: false,
        }
    }
}

impl From<grammar::Period> for Period {
    fn from(period: grammar::Period) -> Self {
        Self {
            duration: period.duration.into(),
        }
    }
}

impl Pattern {
    pub fn from_with_tz(
        reminder_pattern: grammar::ReminderPattern,
        user_timezone: Tz,
    ) -> Result<Self, ()> {
        let now = user_timezone.from_utc_datetime(&now_time()).naive_local();
        match reminder_pattern {
            grammar::ReminderPattern::Recurrence(recurrence) => {
                Ok(Self::Recurrence(Recurrence::from_holey(recurrence, now)?))
            }
            grammar::ReminderPattern::Countdown(countdown) => {
                Ok(Self::Countdown(countdown.into()))
            }
            grammar::ReminderPattern::Period(period) => {
                Ok(Self::Period(period.into()))
            }
        }
    }

    pub fn next(&mut self, cur: NaiveDateTime) -> Option<NaiveDateTime> {
        match self {
            Self::Recurrence(recurrence) => recurrence.next(cur),
            Self::Countdown(countdown) => countdown.next(cur),
            Self::Period(period) => Some(period.next(cur)),
        }
    }
}

#[cfg(test)]
mod test {
    use crate::{grammar::parse_reminder, parsers::test::TEST_TIMESTAMP};

    use super::*;

    lazy_static! {
        static ref TEST_TZ: Tz = "Europe/Amsterdam".parse::<Tz>().unwrap();
        static ref TEST_TIME: DateTime<Tz> =
            TEST_TZ.with_ymd_and_hms(2007, 2, 2, 12, 30, 30).unwrap();
    }

    fn get_all_times(
        mut pattern: Pattern,
    ) -> impl Iterator<Item = NaiveDateTime> {
        let cur = now_time();
        std::iter::successors(Some(cur), move |&cur| pattern.next(cur))
            .skip(1)
            .map(|x| TEST_TZ.from_utc_datetime(&x).naive_local())
    }

    #[test]
    fn test_countdown() {
        let s = "1h2m3s";
        let parsed = parse_reminder(s).unwrap().pattern.unwrap();
        let pattern = Pattern::from_with_tz(parsed, *TEST_TZ).unwrap();
        unsafe {
            TEST_TIMESTAMP = TEST_TIME.timestamp();
        }
        assert_eq!(
            get_all_times(pattern).collect::<Vec<_>>(),
            vec![TEST_TZ
                .with_ymd_and_hms(2007, 2, 2, 13, 32, 33)
                .unwrap()
                .naive_local()]
        );
    }
}
