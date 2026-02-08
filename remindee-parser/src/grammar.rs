use bitmask_enum::bitmask;
use nonempty::{nonempty, NonEmpty};

use pest::{iterators::Pair, Parser};
use pest_derive::Parser;

extern crate alloc;

#[derive(Parser)]
#[grammar = "grammars/reminder.pest"]
struct ReminderParser;

#[derive(Debug, Default)]
pub struct HoleyDate {
    pub year: Option<i32>,
    pub month: Option<u32>,
    pub day: Option<u32>,
}

#[derive(Debug, Default)]
pub struct Interval {
    pub years: i32,
    pub months: u32,
    pub weeks: u32,
    pub days: u32,
    pub hours: u32,
    pub minutes: u32,
    pub seconds: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Weekday {
    Monday,
    Tuesday,
    Wednesday,
    Thursday,
    Friday,
    Saturday,
    Sunday,
}

#[bitmask(u8)]
pub enum Weekdays {
    Monday,
    Tuesday,
    Wednesday,
    Thursday,
    Friday,
    Saturday,
    Sunday,
}

#[derive(Debug)]
pub enum DateDivisor {
    Weekdays(Weekdays),
    Interval(DateInterval),
}

#[derive(Debug)]
pub struct DateRange {
    pub from: HoleyDate,
    pub until: Option<HoleyDate>,
    pub date_divisor: DateDivisor,
}

impl Default for DateRange {
    fn default() -> Self {
        Self {
            date_divisor: DateDivisor::Interval(DateInterval {
                days: 1,
                ..Default::default()
            }),
            from: Default::default(),
            until: None,
        }
    }
}

#[derive(Debug)]
pub enum DatePattern {
    Point(HoleyDate),
    Range(DateRange),
}

#[derive(Debug, Default)]
pub struct Time {
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
}

#[derive(Debug, Default)]
pub struct TimeInterval {
    pub hours: u32,
    pub minutes: u32,
    pub seconds: u32,
}

#[derive(Debug, Default)]
pub struct DateInterval {
    pub years: i32,
    pub months: u32,
    pub weeks: u32,
    pub days: u32,
}

#[derive(Debug, Default)]
pub struct TimeRange {
    pub from: Option<Time>,
    pub until: Option<Time>,
    pub interval: TimeInterval,
}

#[derive(Debug)]
pub enum TimePattern {
    Point(Time),
    Range(TimeRange),
}

#[derive(Debug)]
pub struct Recurrence {
    pub dates_patterns: NonEmpty<DatePattern>,
    pub time_patterns: Vec<TimePattern>,
}

#[derive(Debug, Default)]
pub struct Countdown {
    pub durations: Vec<Interval>,
}

#[derive(Debug)]
pub enum ReminderPattern {
    Recurrence(Recurrence),
    Countdown(Countdown),
    Cron(Cron),
}

#[derive(Debug, Default)]
pub struct Reminder {
    pub description: Option<Description>,
    pub pattern: Option<ReminderPattern>,
    pub nag_interval: Option<Interval>,
}

#[derive(Debug)]
pub struct Cron {
    pub expr: String,
}

#[derive(Debug, Default)]
pub struct Description(pub String);

trait Parse {
    fn parse(pair: Pair<'_, Rule>) -> Option<Self>
    where
        Self: Sized;
}

impl Parse for HoleyDate {
    fn parse(pair: Pair<'_, Rule>) -> Option<Self> {
        let mut holey_date = Self::default();
        for rec in pair.into_inner() {
            match rec.as_rule() {
                Rule::year => {
                    holey_date.year = Some(rec.as_str().parse().ok()?);
                }
                Rule::month => {
                    holey_date.month = Some(rec.as_str().parse().ok()?);
                }
                Rule::day => {
                    holey_date.day = Some(rec.as_str().parse().ok()?);
                }
                _ => unreachable!(),
            }
        }
        Some(holey_date)
    }
}

impl Parse for Interval {
    fn parse(pair: Pair<'_, Rule>) -> Option<Self> {
        let mut interval = Self::default();
        for rec in pair.into_inner() {
            match rec.as_rule() {
                Rule::interval_years => {
                    interval.years = rec.as_str().parse().ok()?;
                }
                Rule::interval_months => {
                    interval.months = rec.as_str().parse().ok()?;
                }
                Rule::interval_weeks => {
                    interval.weeks = rec.as_str().parse().ok()?;
                }
                Rule::interval_days => {
                    interval.days = rec.as_str().parse().ok()?;
                }
                Rule::interval_hours => {
                    interval.hours = rec.as_str().parse().ok()?;
                }
                Rule::interval_minutes => {
                    interval.minutes = rec.as_str().parse().ok()?;
                }
                Rule::interval_seconds => {
                    interval.seconds = rec.as_str().parse().ok()?;
                }
                _ => unreachable!(),
            }
        }
        Some(interval)
    }
}

impl Weekday {
    fn next(&self) -> Self {
        match *self {
            Self::Monday => Self::Tuesday,
            Self::Tuesday => Self::Wednesday,
            Self::Wednesday => Self::Thursday,
            Self::Thursday => Self::Friday,
            Self::Friday => Self::Saturday,
            Self::Saturday => Self::Sunday,
            Self::Sunday => Self::Monday,
        }
    }
}

impl Parse for Weekday {
    fn parse(pair: Pair<'_, Rule>) -> Option<Self> {
        pair.into_inner()
            .next()
            .map(|weekday| match weekday.as_rule() {
                Rule::monday => Self::Monday,
                Rule::tuesday => Self::Tuesday,
                Rule::wednesday => Self::Wednesday,
                Rule::thursday => Self::Thursday,
                Rule::friday => Self::Friday,
                Rule::saturday => Self::Saturday,
                Rule::sunday => Self::Sunday,
                _ => unreachable!(),
            })
    }
}

impl Weekdays {
    fn push(&mut self, weekday: Weekday) {
        *self |= match weekday {
            Weekday::Monday => Self::Monday,
            Weekday::Tuesday => Self::Tuesday,
            Weekday::Wednesday => Self::Wednesday,
            Weekday::Thursday => Self::Thursday,
            Weekday::Friday => Self::Friday,
            Weekday::Saturday => Self::Saturday,
            Weekday::Sunday => Self::Sunday,
        };
    }
}
impl Parse for Weekdays {
    fn parse(pair: Pair<'_, Rule>) -> Option<Self> {
        let mut weekdays = Self::none();
        let mut weekday_range = pair.into_inner();
        let mut weekday_from = weekday_range.next().and_then(Weekday::parse)?;
        let weekday_to = weekday_range
            .next()
            .and_then(Weekday::parse)
            .unwrap_or(weekday_from);
        while weekday_from != weekday_to {
            weekdays.push(weekday_from);
            weekday_from = weekday_from.next();
        }
        weekdays.push(weekday_from);
        Some(weekdays)
    }
}

impl Parse for DateRange {
    fn parse(pair: Pair<'_, Rule>) -> Option<Self> {
        let mut date_range = Self::default();
        for rec in pair.into_inner() {
            match rec.as_rule() {
                Rule::date_from => {
                    date_range.from = HoleyDate::parse(rec)?;
                }
                Rule::date_until => {
                    date_range.until = Some(HoleyDate::parse(rec)?);
                }
                Rule::date_interval => {
                    date_range.date_divisor =
                        DateDivisor::Interval(DateInterval::parse(rec)?);
                }
                Rule::weekdays_range => {
                    let weekdays = match date_range.date_divisor {
                        DateDivisor::Weekdays(ref mut w) => w,
                        _ => {
                            date_range.date_divisor =
                                DateDivisor::Weekdays(Weekdays::none());
                            match date_range.date_divisor {
                                DateDivisor::Weekdays(ref mut w) => w,
                                _ => unreachable!(),
                            }
                        }
                    };
                    *weekdays |= Weekdays::parse(rec)?;
                }
                _ => unreachable!(),
            }
        }
        Some(date_range)
    }
}

impl Parse for Time {
    fn parse(pair: Pair<'_, Rule>) -> Option<Self> {
        let mut time = Self::default();
        for time_component in pair.into_inner() {
            match time_component.as_rule() {
                Rule::hour => {
                    time.hour = time_component.as_str().parse().ok()?;
                }
                Rule::minute => {
                    time.minute = time_component.as_str().parse().ok()?;
                }
                Rule::second => {
                    time.second = time_component.as_str().parse().ok()?;
                }
                _ => unreachable!(),
            }
        }
        Some(time)
    }
}

impl Parse for TimeInterval {
    fn parse(pair: Pair<'_, Rule>) -> Option<Self> {
        let mut time_interval = Self::default();
        for rec in pair.into_inner() {
            match rec.as_rule() {
                Rule::interval_hours => {
                    time_interval.hours = rec.as_str().parse().ok()?;
                }
                Rule::interval_minutes => {
                    time_interval.minutes = rec.as_str().parse().ok()?;
                }
                Rule::interval_seconds => {
                    time_interval.seconds = rec.as_str().parse().ok()?;
                }
                _ => unreachable!(),
            }
        }
        Some(time_interval)
    }
}

impl Parse for DateInterval {
    fn parse(pair: Pair<'_, Rule>) -> Option<Self> {
        let mut date_interval = Self::default();
        for rec in pair.into_inner() {
            match rec.as_rule() {
                Rule::interval_years => {
                    date_interval.years = rec.as_str().parse().ok()?;
                }
                Rule::interval_months => {
                    date_interval.months = rec.as_str().parse().ok()?;
                }
                Rule::interval_weeks => {
                    date_interval.weeks = rec.as_str().parse().ok()?;
                }
                Rule::interval_days => {
                    date_interval.days = rec.as_str().parse().ok()?;
                }
                _ => unreachable!(),
            }
        }
        Some(date_interval)
    }
}

impl Parse for TimeRange {
    fn parse(pair: Pair<'_, Rule>) -> Option<Self> {
        let mut time_range = Self::default();
        for rec in pair.into_inner() {
            match rec.as_rule() {
                Rule::time_from => {
                    time_range.from = Some(Time::parse(rec)?);
                }
                Rule::time_until => {
                    time_range.until = Some(Time::parse(rec)?);
                }
                Rule::time_interval => {
                    time_range.interval = TimeInterval::parse(rec)?;
                }
                _ => unreachable!(),
            }
        }
        Some(time_range)
    }
}

impl Default for Recurrence {
    fn default() -> Self {
        // make sure there's at least one date range
        // the inserted holey range will correspond to the current date point
        Self {
            dates_patterns: nonempty![DatePattern::Point(HoleyDate::default())],
            time_patterns: vec![],
        }
    }
}

impl Parse for Recurrence {
    fn parse(pair: Pair<'_, Rule>) -> Option<Self> {
        let mut recurrence = Self::default();
        for rec in pair.into_inner() {
            match rec.as_rule() {
                Rule::dates_point => {
                    recurrence
                        .dates_patterns
                        .push(DatePattern::Point(HoleyDate::parse(rec)?));
                }
                Rule::dates_range => {
                    recurrence
                        .dates_patterns
                        .push(DatePattern::Range(DateRange::parse(rec)?));
                }
                Rule::time_point => {
                    recurrence
                        .time_patterns
                        .push(TimePattern::Point(Time::parse(rec)?));
                }
                Rule::time_range => {
                    recurrence
                        .time_patterns
                        .push(TimePattern::Range(TimeRange::parse(rec)?));
                }
                _ => unreachable!(),
            }
        }
        if recurrence.dates_patterns.len() > 1 {
            recurrence.dates_patterns =
                NonEmpty::from_vec(recurrence.dates_patterns.tail).unwrap();
        }
        Some(recurrence)
    }
}

impl Parse for Countdown {
    fn parse(pair: Pair<'_, Rule>) -> Option<Self> {
        let mut countdown = Self::default();
        for rec in pair.into_inner() {
            match rec.as_rule() {
                Rule::interval => {
                    countdown.durations.push(Interval::parse(rec)?);
                }
                _ => unreachable!(),
            }
        }
        Some(countdown)
    }
}

impl Parse for Cron {
    fn parse(pair: Pair<'_, Rule>) -> Option<Self> {
        for rec in pair.into_inner() {
            if rec.as_rule() == Rule::cron_expr {
                return Some(Self {
                    expr: rec.as_str().to_string(),
                });
            }
        }
        None
    }
}

impl Parse for Description {
    fn parse(pair: Pair<'_, Rule>) -> Option<Self> {
        Some(Self(pair.as_str().to_string()))
    }
}

impl Parse for Reminder {
    fn parse(pair: Pair<'_, Rule>) -> Option<Self> {
        let mut reminder = Self::default();
        for rec in pair.into_inner() {
            match rec.as_rule() {
                Rule::description => {
                    reminder.description = Some(Description::parse(rec)?);
                }
                Rule::recurrence => {
                    reminder.pattern = Some(ReminderPattern::Recurrence(
                        Recurrence::parse(rec)?,
                    ));
                }
                Rule::countdown => {
                    reminder.pattern = Some(ReminderPattern::Countdown(
                        Countdown::parse(rec)?,
                    ));
                }
                Rule::cron => {
                    reminder.pattern =
                        Some(ReminderPattern::Cron(Cron::parse(rec)?));
                }
                Rule::nag_suffix => {
                    let nag_interval = rec.into_inner().find_map(|inner| {
                        if inner.as_rule() == Rule::interval {
                            Interval::parse(inner)
                        } else {
                            None
                        }
                    })?;
                    reminder.nag_interval = Some(nag_interval);
                }
                Rule::EOI => {}
                _ => unreachable!(),
            }
        }
        Some(reminder)
    }
}

pub fn parse_reminder(s: &str) -> Option<Reminder> {
    Reminder::parse(
        ReminderParser::parse(Rule::reminder, s)
            .map_err(|err| {
                log::debug!("{}", err);
            })
            .ok()?
            .next()?,
    )
}
