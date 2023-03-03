use std::cmp::max;
use std::fmt::Formatter;

use bitmask_enum::bitmask;
use chrono::offset::TimeZone;
use chrono::prelude::*;
use chrono::Duration;
use chronoutil::{shift_months, shift_years};
use nonempty::NonEmpty;
use serde::{Deserialize, Serialize};

use crate::date;
use crate::grammar;
use crate::parsers::now_time;

#[derive(Debug)]
pub struct Tz(chrono_tz::Tz);

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
    #[serde(rename = "tz")]
    pub timezone: Tz,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Countdown {
    #[serde(rename = "dur")]
    pub duration: Interval,
    #[serde(rename = "tz")]
    pub timezone: Tz,
    used: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Pattern {
    Recurrence(Recurrence),
    Countdown(Countdown),
}

trait DateDisplay {
    fn relfmt<D: Datelike>(
        &self,
        f: &mut Formatter<'_>,
        now: &D,
    ) -> Result<bool, std::fmt::Error>;
}

pub fn fill_date_holes(
    holey_date: &grammar::HoleyDate,
    lower_bound: NaiveDate,
) -> Option<NaiveDate> {
    let year = holey_date.year.unwrap_or(lower_bound.year());
    let month = holey_date.month.unwrap_or(lower_bound.month());
    let day = holey_date.day.unwrap_or(lower_bound.day());
    let day = date::normalise_day(year, month, day);
    let date = NaiveDate::from_ymd_opt(year, month, day)?;
    if date >= lower_bound {
        return Some(date);
    }
    if holey_date.day.is_none() && date + Duration::days(1) > lower_bound {
        Some(date + Duration::days(1))
    } else if holey_date.month.is_none() && shift_months(date, 1) > lower_bound
    {
        Some(shift_months(date, 1))
    } else if shift_years(date, 1) > lower_bound {
        Some(shift_years(date, 1))
    } else {
        None
    }
}

impl Serialize for Tz {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.0.name())
    }
}

impl<'de> Deserialize<'de> for Tz {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        let tz = s.parse().map_err(serde::de::Error::custom)?;
        Ok(Self(tz))
    }
}

impl Tz {
    fn local_to_utc(&self, time: &NaiveDateTime) -> Option<NaiveDateTime> {
        self.0
            .from_local_datetime(time)
            .earliest()
            .map(|dt| dt.naive_utc())
    }
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

impl DateRange {
    pub fn get_nearest_date(&self, date: NaiveDate) -> Option<NaiveDate> {
        match self.date_divisor {
            DateDivisor::Weekdays(weekdays) => {
                let weekdays = (0..7)
                    .filter(|i| weekdays.bits() & (1 << i) != 0)
                    .collect::<Vec<_>>();
                let nearest_date = date::find_nearest_weekday(
                    max(date, self.from),
                    NonEmpty::from_vec(weekdays).unwrap(),
                );
                if self
                    .until
                    .map(|until| nearest_date <= until)
                    .unwrap_or(true)
                {
                    Some(nearest_date)
                } else {
                    None
                }
            }
            DateDivisor::Interval(int) => {
                let mut nearest_date = self.from;
                while nearest_date < date {
                    nearest_date = date::add_date_interval(nearest_date, &int);
                }
                if self
                    .until
                    .map(|until| nearest_date <= until)
                    .unwrap_or(true)
                {
                    Some(nearest_date)
                } else {
                    None
                }
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
}

impl Recurrence {
    pub fn from_with_tz(
        recurrence: grammar::Recurrence,
        tz: chrono_tz::Tz,
    ) -> Result<Self, ()> {
        let lower_bound = tz.from_utc_datetime(&now_time()).naive_local();
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
            .filter(|time_pattern| match time_pattern {
                grammar::TimePattern::Point(_) => false,
                grammar::TimePattern::Range(_) => true,
            })
            .count()
            > 0;
        let init_time = fill_date_holes(first_date, lower_bound.date())
            .map(|date| date.and_time(first_time))
            .ok_or(())?;
        let init_time =
            if init_time < lower_bound && !has_divisor && !has_time_divisor {
                if first_date.day.is_none() {
                    init_time + Duration::days(1)
                } else if first_date.month.is_none() {
                    shift_months(init_time, 1)
                } else {
                    shift_years(init_time, 1)
                }
            } else {
                init_time
            };
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
            timezone: Tz(tz),
        })
    }

    pub fn next(&self, cur: NaiveDateTime) -> Option<NaiveDateTime> {
        let cur = self.timezone.0.from_utc_datetime(&cur).naive_local();
        let cur_date = cur.date();
        let cur_time = cur.time();
        let first_date = self
            .dates_patterns
            .iter()
            .flat_map(|pattern| match pattern {
                &DatePattern::Point(date) => Some(date),
                DatePattern::Range(ref range) => {
                    range.get_nearest_date(cur_date)
                }
            })
            .min()?;
        let first_time = self
            .time_patterns
            .iter()
            .map(|pattern| match pattern {
                &TimePattern::Point(time) => time,
                TimePattern::Range(ref range) => range
                    .from
                    .unwrap_or(NaiveTime::from_hms_opt(0, 0, 0).unwrap()),
            })
            .min()?;
        if first_date > cur_date {
            return self
                .timezone
                .local_to_utc(&first_date.and_time(first_time));
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
                                .map(|time_until| next_time <= time_until)
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
            return self.timezone.local_to_utc(&cur_date.and_time(next_time));
        }
        let next_date = self
            .dates_patterns
            .iter()
            .filter(|&int| match int {
                &DatePattern::Point(date) => date > cur_date,
                DatePattern::Range(ref range) => range
                    .until
                    .map(|date_until| date_until > cur_date)
                    .unwrap_or(true),
            })
            .flat_map(|int| match int {
                &DatePattern::Point(date) => Some(date),
                DatePattern::Range(ref range) => {
                    let from = range.from;
                    if from > cur_date {
                        Some(from)
                    } else {
                        let next_date = range
                            .get_nearest_date(cur_date + Duration::days(1))?;
                        if range
                            .until
                            .map(|date_until| next_date <= date_until)
                            .unwrap_or(true)
                        {
                            Some(next_date)
                        } else {
                            None
                        }
                    }
                }
            })
            .min();

        next_date
            .map(|next_date| next_date.and_time(first_time))
            .and_then(|next_dt| self.timezone.local_to_utc(&next_dt))
    }
}

impl Countdown {
    pub fn next(&mut self, cur: NaiveDateTime) -> Option<NaiveDateTime> {
        let cur = self.timezone.0.from_utc_datetime(&cur).naive_local();
        if self.used {
            None
        } else {
            self.used = true;
            let next_time = date::add_interval(cur, &self.duration);
            self.timezone.local_to_utc(&next_time)
        }
    }
}

impl Countdown {
    fn from_with_tz(countdown: grammar::Countdown, tz: chrono_tz::Tz) -> Self {
        Self {
            duration: countdown.duration.into(),
            timezone: Tz(tz),
            used: false,
        }
    }
}

impl Pattern {
    pub fn from_with_tz(
        reminder_pattern: grammar::ReminderPattern,
        tz: chrono_tz::Tz,
    ) -> Result<Self, ()> {
        match reminder_pattern {
            grammar::ReminderPattern::Recurrence(recurrence) => {
                Ok(Self::Recurrence(Recurrence::from_with_tz(recurrence, tz)?))
            }
            grammar::ReminderPattern::Countdown(countdown) => {
                Ok(Self::Countdown(Countdown::from_with_tz(countdown, tz)))
            }
        }
    }

    pub fn next(&mut self, cur: NaiveDateTime) -> Option<NaiveDateTime> {
        match self {
            Self::Recurrence(recurrence) => recurrence.next(cur),
            Self::Countdown(countdown) => countdown.next(cur),
        }
    }
}

impl std::fmt::Display for Pattern {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Recurrence(recurrence) => write!(f, "{}", recurrence),
            Self::Countdown(countdown) => write!(f, "{}", countdown),
        }
    }
}

impl std::fmt::Display for Recurrence {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let now = self.timezone.0.from_utc_datetime(&now_time());
        if self.time_patterns.len() == 1
            && self.dates_patterns.len() == 1
            && matches!(self.time_patterns[0], TimePattern::Point(_))
            && matches!(self.dates_patterns[0], DatePattern::Point(_))
        {
            return Ok(());
        }
        let mut nonempty = false;
        for (i, dates_pattern) in self.dates_patterns.iter().enumerate() {
            if i != 0 {
                write!(f, ",")?;
            }
            nonempty |= dates_pattern.relfmt(f, &now)?;
        }
        if nonempty {
            write!(f, " ")?;
        }
        for (i, time_pattern) in self.time_patterns.iter().enumerate() {
            if i != 0 {
                write!(f, ",")?;
            }
            write!(f, "{}", time_pattern)?;
        }
        Ok(())
    }
}

impl std::fmt::Display for TimePattern {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Point(time) => {
                write!(f, "{:02}:{:02}", time.hour(), time.minute())
            }
            Self::Range(range) => range.fmt(f),
        }
    }
}

impl DateDisplay for DatePattern {
    fn relfmt<D: Datelike>(
        &self,
        f: &mut Formatter<'_>,
        now: &D,
    ) -> Result<bool, std::fmt::Error> {
        match self {
            Self::Point(date) => date.relfmt(f, now),
            Self::Range(range) => range.relfmt(f, now),
        }
    }
}

impl std::fmt::Display for TimeRange {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if let Some(from) = self.from {
            write!(f, "{:02}:{:02}", from.hour(), from.minute())?;
        }
        write!(f, "—")?;
        if let Some(until) = self.until {
            write!(f, "{:02}:{:02}", until.hour(), until.minute())?;
        }
        write!(f, "/")?;
        self.interval.fmt(f)?;
        Ok(())
    }
}

impl DateDisplay for DateRange {
    fn relfmt<D: Datelike>(
        &self,
        f: &mut Formatter<'_>,
        now: &D,
    ) -> Result<bool, std::fmt::Error> {
        self.from.relfmt(f, now)?;
        write!(f, "—")?;
        if let Some(until) = self.until {
            if self.from != until {
                until.relfmt(f, now)?;
            }
        }
        if !matches!(
            self.date_divisor,
            DateDivisor::Interval(DateInterval {
                years: 0,
                months: 0,
                weeks: 0,
                days: 1,
            })
        ) {
            write!(f, "/{}", self.date_divisor)?;
        }
        Ok(true)
    }
}

impl DateDisplay for NaiveDate {
    fn relfmt<D: Datelike>(
        &self,
        f: &mut Formatter<'_>,
        now: &D,
    ) -> Result<bool, std::fmt::Error> {
        let same_year = self.year() == now.year();
        let same_month = same_year && self.month() == now.month();
        let same_day = same_month && self.day() == now.day();
        if same_day {
            Ok(false)
        } else {
            if same_month {
                write!(f, "{}", self.format("%d"))?;
            } else if same_year {
                write!(f, "{}", self.format("%d.%m"))?;
            } else {
                write!(f, "{}", self.format("%d.%m.%y"))?;
            }
            Ok(true)
        }
    }
}

impl std::fmt::Display for DateDivisor {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match *self {
            DateDivisor::Weekdays(weekdays) => weekdays.fmt(f),
            DateDivisor::Interval(interval) => interval.fmt(f),
        }
    }
}

impl std::fmt::Display for Weekdays {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let weekdays = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"]
            .iter()
            .enumerate()
            .filter_map(|(i, w)| {
                if self.bits() & (1 << i) != 0 {
                    Some(w)
                } else {
                    None
                }
            });
        for (i, weekday) in weekdays.enumerate() {
            if i != 0 {
                write!(f, ",")?;
            }
            write!(f, "{}", weekday)?;
        }
        Ok(())
    }
}

impl std::fmt::Display for TimeInterval {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if self.hours != 0 {
            write!(f, "{}h", self.hours)?;
        }
        if self.minutes != 0 {
            write!(f, "{}m", self.minutes)?;
        }
        if self.seconds != 0 {
            write!(f, "{}s", self.seconds)?;
        }
        Ok(())
    }
}

impl std::fmt::Display for DateInterval {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if self.years != 0 {
            write!(f, "{}s", self.years)?;
        }
        if self.months != 0 {
            write!(f, "{}m", self.months)?;
        }
        if self.weeks != 0 {
            write!(f, "{}s", self.weeks)?;
        }
        if self.days != 0 {
            write!(f, "{}d", self.days)?;
        }
        Ok(())
    }
}

impl std::fmt::Display for Interval {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if self.years != 0 {
            write!(f, "{}s", self.years)?;
        }
        if self.months != 0 {
            write!(f, "{}mo", self.months)?;
        }
        if self.weeks != 0 {
            write!(f, "{}s", self.weeks)?;
        }
        if self.days != 0 {
            write!(f, "{}d", self.days)?;
        }
        if self.hours != 0 {
            write!(f, "{}h", self.hours)?;
        }
        if self.minutes != 0 {
            write!(f, "{}m", self.minutes)?;
        }
        if self.seconds != 0 {
            write!(f, "{}s", self.seconds)?;
        }
        Ok(())
    }
}

impl std::fmt::Display for Countdown {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.duration.fmt(f)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        grammar::parse_reminder,
        parsers::test::{TEST_TIME, TEST_TIMESTAMP, TEST_TZ},
    };

    fn get_all_times(
        mut pattern: Pattern,
    ) -> impl Iterator<Item = NaiveDateTime> {
        let cur = now_time();
        std::iter::successors(Some(cur), move |&cur| pattern.next(cur))
            .skip(1)
            .map(|x| TEST_TZ.from_utc_datetime(&x).naive_local())
    }

    fn tz(
        year: i32,
        month: u32,
        day: u32,
        hour: u32,
        min: u32,
        sec: u32,
    ) -> NaiveDateTime {
        TEST_TZ
            .with_ymd_and_hms(year, month, day, hour, min, sec)
            .unwrap()
            .naive_local()
    }

    #[test]
    fn test_countdown() {
        unsafe {
            TEST_TIMESTAMP = TEST_TIME.timestamp();
        }
        let s = "1w1h2m3s countdown";
        let parsed_rem = parse_reminder(s).unwrap();
        assert_eq!(
            parsed_rem.description.map(|x| x.0),
            Some("countdown".to_owned())
        );
        let parsed = parsed_rem.pattern.unwrap();
        let pattern = Pattern::from_with_tz(parsed, *TEST_TZ).unwrap();
        assert_eq!(
            get_all_times(pattern).collect::<Vec<_>>(),
            vec![tz(2007, 2, 9, 13, 32, 33)]
        );
    }

    #[test]
    fn test_periodic() {
        unsafe {
            TEST_TIMESTAMP = TEST_TIME.timestamp();
        }
        let s = "- 11-18/1h periodic";
        let parsed_rem = parse_reminder(s).unwrap();
        assert_eq!(
            parsed_rem.description.map(|x| x.0),
            Some("periodic".to_owned())
        );
        let parsed = parsed_rem.pattern.unwrap();
        let pattern = Pattern::from_with_tz(parsed, *TEST_TZ).unwrap();
        assert_eq!(
            get_all_times(pattern).take(15).collect::<Vec<_>>(),
            vec![
                tz(2007, 2, 2, 13, 0, 0),
                tz(2007, 2, 2, 14, 0, 0),
                tz(2007, 2, 2, 15, 0, 0),
                tz(2007, 2, 2, 16, 0, 0),
                tz(2007, 2, 2, 17, 0, 0),
                tz(2007, 2, 2, 18, 0, 0),
                tz(2007, 2, 3, 11, 0, 0),
                tz(2007, 2, 3, 12, 0, 0),
                tz(2007, 2, 3, 13, 0, 0),
                tz(2007, 2, 3, 14, 0, 0),
                tz(2007, 2, 3, 15, 0, 0),
                tz(2007, 2, 3, 16, 0, 0),
                tz(2007, 2, 3, 17, 0, 0),
                tz(2007, 2, 3, 18, 0, 0),
                tz(2007, 2, 4, 11, 0, 0),
            ]
        );
    }

    #[test]
    fn test_date_range() {
        unsafe {
            TEST_TIMESTAMP = TEST_TIME.timestamp();
        }
        let s = "3-6/2d 13:37 date range";
        let parsed_rem = parse_reminder(s).unwrap();
        assert_eq!(
            parsed_rem.description.map(|x| x.0),
            Some("date range".to_owned())
        );
        let parsed = parsed_rem.pattern.unwrap();
        let pattern = Pattern::from_with_tz(parsed, *TEST_TZ).unwrap();
        assert_eq!(
            get_all_times(pattern).collect::<Vec<_>>(),
            vec![tz(2007, 2, 3, 13, 37, 0), tz(2007, 2, 5, 13, 37, 0),]
        );
    }

    #[test]
    fn test_date_format1() {
        unsafe {
            TEST_TIMESTAMP = TEST_TIME.timestamp();
        }
        let s = "07.06.2025 13:37";
        let parsed_rem = parse_reminder(s).unwrap();
        assert_eq!(parsed_rem.description.map(|x| x.0), None);
        let parsed = parsed_rem.pattern.unwrap();
        let pattern = Pattern::from_with_tz(parsed, *TEST_TZ).unwrap();
        assert_eq!(
            get_all_times(pattern).collect::<Vec<_>>(),
            vec![tz(2025, 6, 7, 13, 37, 0)]
        );
    }

    #[test]
    fn test_date_format2() {
        unsafe {
            TEST_TIMESTAMP = TEST_TIME.timestamp();
        }
        let s = "2025/06/07 13:37 date format2";
        let parsed_rem = parse_reminder(s).unwrap();
        assert_eq!(
            parsed_rem.description.map(|x| x.0),
            Some("date format2".to_owned())
        );
        let parsed = parsed_rem.pattern.unwrap();
        let pattern = Pattern::from_with_tz(parsed, *TEST_TZ).unwrap();
        assert_eq!(
            get_all_times(pattern).collect::<Vec<_>>(),
            vec![tz(2025, 6, 7, 13, 37, 0)]
        );
    }

    #[test]
    fn test_end_of_month_increment() {
        unsafe {
            TEST_TIMESTAMP = TEST_TIME.timestamp();
        }
        let s = "12/31/1MONTH 13:37 end of month";
        let parsed_rem = parse_reminder(s).unwrap();
        assert_eq!(
            parsed_rem.description.map(|x| x.0),
            Some("end of month".to_owned())
        );
        let parsed = parsed_rem.pattern.unwrap();
        let pattern = Pattern::from_with_tz(parsed, *TEST_TZ).unwrap();
        assert_eq!(
            get_all_times(pattern).take(16).collect::<Vec<_>>(),
            vec![
                tz(2007, 12, 31, 13, 37, 0),
                tz(2008, 1, 31, 13, 37, 0),
                tz(2008, 2, 29, 13, 37, 0),
                tz(2008, 3, 29, 13, 37, 0),
                tz(2008, 4, 29, 13, 37, 0),
                tz(2008, 5, 29, 13, 37, 0),
                tz(2008, 6, 29, 13, 37, 0),
                tz(2008, 7, 29, 13, 37, 0),
                tz(2008, 8, 29, 13, 37, 0),
                tz(2008, 9, 29, 13, 37, 0),
                tz(2008, 10, 29, 13, 37, 0),
                tz(2008, 11, 29, 13, 37, 0),
                tz(2008, 12, 29, 13, 37, 0),
                tz(2009, 1, 29, 13, 37, 0),
                tz(2009, 2, 28, 13, 37, 0),
                tz(2009, 3, 28, 13, 37, 0),
            ]
        );
    }

    #[test]
    fn test_weekdays() {
        unsafe {
            TEST_TIMESTAMP = TEST_TIME.timestamp();
        }
        let s = "/fri,mon 11:00 weekdays";
        let parsed_rem = parse_reminder(s).unwrap();
        assert_eq!(
            parsed_rem.description.map(|x| x.0),
            Some("weekdays".to_owned())
        );
        let parsed = parsed_rem.pattern.unwrap();
        let pattern = Pattern::from_with_tz(parsed, *TEST_TZ).unwrap();
        assert_eq!(
            get_all_times(pattern).take(4).collect::<Vec<_>>(),
            vec![
                tz(2007, 2, 5, 11, 0, 0),
                tz(2007, 2, 9, 11, 0, 0),
                tz(2007, 2, 12, 11, 0, 0),
                tz(2007, 2, 16, 11, 0, 0),
            ]
        );
    }

    #[test]
    fn test_weekdays_ranges() {
        unsafe {
            TEST_TIMESTAMP = TEST_TIME.timestamp();
        }
        let s = "/fri-mon,wed 15:00:20 weekdays ranges";
        let parsed_rem = parse_reminder(s).unwrap();
        assert_eq!(
            parsed_rem.description.map(|x| x.0),
            Some("weekdays ranges".to_owned())
        );
        let parsed = parsed_rem.pattern.unwrap();
        let pattern = Pattern::from_with_tz(parsed, *TEST_TZ).unwrap();
        assert_eq!(
            get_all_times(pattern).take(10).collect::<Vec<_>>(),
            vec![
                tz(2007, 2, 2, 15, 0, 20),
                tz(2007, 2, 3, 15, 0, 20),
                tz(2007, 2, 4, 15, 0, 20),
                tz(2007, 2, 5, 15, 0, 20),
                tz(2007, 2, 7, 15, 0, 20),
                tz(2007, 2, 9, 15, 0, 20),
                tz(2007, 2, 10, 15, 0, 20),
                tz(2007, 2, 11, 15, 0, 20),
                tz(2007, 2, 12, 15, 0, 20),
                tz(2007, 2, 14, 15, 0, 20),
            ]
        );
    }

    #[test]
    fn test_description_trim() {
        unsafe {
            TEST_TIMESTAMP = TEST_TIME.timestamp();
        }
        let s = "15:16     test    description   ";
        let parsed_rem = parse_reminder(s).unwrap();
        assert_eq!(
            parsed_rem.description.map(|x| x.0),
            Some("test    description".to_owned())
        );
        let parsed = parsed_rem.pattern.unwrap();
        let pattern = Pattern::from_with_tz(parsed, *TEST_TZ).unwrap();
        assert_eq!(
            get_all_times(pattern).collect::<Vec<_>>(),
            vec![tz(2007, 2, 2, 15, 16, 0),]
        );
    }

    #[test]
    fn test_date_range_weekends() {
        unsafe {
            TEST_TIMESTAMP = TEST_TIME.timestamp();
        }
        let s = "10-20/mon,fri-sun 11-12/1h date range weekends";
        let parsed_rem = parse_reminder(s).unwrap();
        assert_eq!(
            parsed_rem.description.map(|x| x.0),
            Some("date range weekends".to_owned())
        );
        let parsed = parsed_rem.pattern.unwrap();
        let pattern = Pattern::from_with_tz(parsed, *TEST_TZ).unwrap();
        assert_eq!(
            get_all_times(pattern).collect::<Vec<_>>(),
            vec![
                tz(2007, 2, 10, 11, 0, 0),
                tz(2007, 2, 10, 12, 0, 0),
                tz(2007, 2, 11, 11, 0, 0),
                tz(2007, 2, 11, 12, 0, 0),
                tz(2007, 2, 12, 11, 0, 0),
                tz(2007, 2, 12, 12, 0, 0),
                tz(2007, 2, 16, 11, 0, 0),
                tz(2007, 2, 16, 12, 0, 0),
                tz(2007, 2, 17, 11, 0, 0),
                tz(2007, 2, 17, 12, 0, 0),
                tz(2007, 2, 18, 11, 0, 0),
                tz(2007, 2, 18, 12, 0, 0),
                tz(2007, 2, 19, 11, 0, 0),
                tz(2007, 2, 19, 12, 0, 0),
            ]
        );
    }

    #[test]
    fn test_date_range_over_year() {
        unsafe {
            TEST_TIMESTAMP = TEST_TIME.timestamp();
        }
        let s = "12/16-3/16/1m 18:15 date range over year";
        let parsed_rem = parse_reminder(s).unwrap();
        assert_eq!(
            parsed_rem.description.map(|x| x.0),
            Some("date range over year".to_owned())
        );
        let parsed = parsed_rem.pattern.unwrap();
        let pattern = Pattern::from_with_tz(parsed, *TEST_TZ).unwrap();
        dbg!(&pattern);
        assert_eq!(
            get_all_times(pattern).collect::<Vec<_>>(),
            vec![
                tz(2007, 12, 16, 18, 15, 0),
                tz(2008, 1, 16, 18, 15, 0),
                tz(2008, 2, 16, 18, 15, 0),
                tz(2008, 3, 16, 18, 15, 0),
            ]
        );
    }
}
