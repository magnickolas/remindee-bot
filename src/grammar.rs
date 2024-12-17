use bitmask_enum::bitmask;
use nonempty::{nonempty, NonEmpty};

use pest::{iterators::Pair, Parser};

#[derive(Parser)]
#[grammar = "grammars/reminder.pest"]
struct ReminderParser;

#[derive(Debug, Default)]
pub(crate) struct HoleyDate {
    pub(crate) year: Option<i32>,
    pub(crate) month: Option<u32>,
    pub(crate) day: Option<u32>,
}

#[derive(Debug, Default)]
pub(crate) struct Interval {
    pub(crate) years: i32,
    pub(crate) months: u32,
    pub(crate) weeks: u32,
    pub(crate) days: u32,
    pub(crate) hours: u32,
    pub(crate) minutes: u32,
    pub(crate) seconds: u32,
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
pub(crate) enum Weekdays {
    Monday,
    Tuesday,
    Wednesday,
    Thursday,
    Friday,
    Saturday,
    Sunday,
}

#[derive(Debug)]
pub(crate) enum DateDivisor {
    Weekdays(Weekdays),
    Interval(DateInterval),
}

#[derive(Debug)]
pub(crate) struct DateRange {
    pub(crate) from: HoleyDate,
    pub(crate) until: Option<HoleyDate>,
    pub(crate) date_divisor: DateDivisor,
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
pub(crate) enum DatePattern {
    Point(HoleyDate),
    Range(DateRange),
}

#[derive(Debug, Default)]
pub(crate) struct Time {
    pub(crate) hour: u32,
    pub(crate) minute: u32,
    pub(crate) second: u32,
}

#[derive(Debug, Default)]
pub(crate) struct TimeInterval {
    pub(crate) hours: u32,
    pub(crate) minutes: u32,
    pub(crate) seconds: u32,
}

#[derive(Debug, Default)]
pub(crate) struct DateInterval {
    pub(crate) years: i32,
    pub(crate) months: u32,
    pub(crate) weeks: u32,
    pub(crate) days: u32,
}

#[derive(Debug, Default)]
pub(crate) struct TimeRange {
    pub(crate) from: Option<Time>,
    pub(crate) until: Option<Time>,
    pub(crate) interval: TimeInterval,
}

#[derive(Debug)]
pub(crate) enum TimePattern {
    Point(Time),
    Range(TimeRange),
}

#[derive(Debug)]
pub(crate) struct Recurrence {
    pub(crate) dates_patterns: NonEmpty<DatePattern>,
    pub(crate) time_patterns: Vec<TimePattern>,
}

#[derive(Debug, Default)]
pub(crate) struct Countdown {
    pub(crate) durations: Vec<Interval>,
}

#[derive(Debug)]
pub(crate) enum ReminderPattern {
    Recurrence(Recurrence),
    Countdown(Countdown),
}

#[derive(Debug, Default)]
pub(crate) struct Reminder {
    pub(crate) description: Option<Description>,
    pub(crate) pattern: Option<ReminderPattern>,
}

#[derive(Debug, Default)]
pub(crate) struct Description(pub(crate) String);

trait Parse {
    fn parse(pair: Pair<'_, Rule>) -> Result<Self, ()>
    where
        Self: Sized;
}

impl Parse for HoleyDate {
    fn parse(pair: Pair<'_, Rule>) -> Result<Self, ()> {
        let mut holey_date = Self::default();
        for rec in pair.into_inner() {
            match rec.as_rule() {
                Rule::year => {
                    holey_date.year =
                        Some(rec.as_str().parse().map_err(|_| ())?);
                }
                Rule::month => {
                    holey_date.month =
                        Some(rec.as_str().parse().map_err(|_| ())?);
                }
                Rule::day => {
                    holey_date.day =
                        Some(rec.as_str().parse().map_err(|_| ())?);
                }
                _ => unreachable!(),
            }
        }
        Ok(holey_date)
    }
}

impl Parse for Interval {
    fn parse(pair: Pair<'_, Rule>) -> Result<Self, ()> {
        let mut interval = Self::default();
        for rec in pair.into_inner() {
            match rec.as_rule() {
                Rule::interval_years => {
                    interval.years = rec.as_str().parse().map_err(|_| ())?;
                }
                Rule::interval_months => {
                    interval.months = rec.as_str().parse().map_err(|_| ())?;
                }
                Rule::interval_weeks => {
                    interval.weeks = rec.as_str().parse().map_err(|_| ())?;
                }
                Rule::interval_days => {
                    interval.days = rec.as_str().parse().map_err(|_| ())?;
                }
                Rule::interval_hours => {
                    interval.hours = rec.as_str().parse().map_err(|_| ())?;
                }
                Rule::interval_minutes => {
                    interval.minutes = rec.as_str().parse().map_err(|_| ())?;
                }
                Rule::interval_seconds => {
                    interval.seconds = rec.as_str().parse().map_err(|_| ())?;
                }
                _ => unreachable!(),
            }
        }
        Ok(interval)
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
    fn parse(pair: Pair<'_, Rule>) -> Result<Self, ()> {
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
            .ok_or(())
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
    fn parse(pair: Pair<'_, Rule>) -> Result<Self, ()> {
        let mut weekdays = Self::none();
        let mut weekday_range = pair.into_inner();
        let mut weekday_from = weekday_range
            .next()
            .map(Weekday::parse)
            .transpose()?
            .ok_or(())?;
        let weekday_to = weekday_range
            .next()
            .map(Weekday::parse)
            .transpose()?
            .unwrap_or(weekday_from);
        while weekday_from != weekday_to {
            weekdays.push(weekday_from);
            weekday_from = weekday_from.next();
        }
        weekdays.push(weekday_from);
        Ok(weekdays)
    }
}

impl Parse for DateRange {
    fn parse(pair: Pair<'_, Rule>) -> Result<Self, ()> {
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
        Ok(date_range)
    }
}

impl Parse for Time {
    fn parse(pair: Pair<'_, Rule>) -> Result<Self, ()> {
        let mut time = Self::default();
        for time_component in pair.into_inner() {
            match time_component.as_rule() {
                Rule::hour => {
                    time.hour =
                        time_component.as_str().parse().map_err(|_| ())?;
                }
                Rule::minute => {
                    time.minute =
                        time_component.as_str().parse().map_err(|_| ())?;
                }
                Rule::second => {
                    time.second =
                        time_component.as_str().parse().map_err(|_| ())?;
                }
                _ => unreachable!(),
            }
        }
        Ok(time)
    }
}

impl Parse for TimeInterval {
    fn parse(pair: Pair<'_, Rule>) -> Result<Self, ()> {
        let mut time_interval = Self::default();
        for rec in pair.into_inner() {
            match rec.as_rule() {
                Rule::interval_hours => {
                    time_interval.hours =
                        rec.as_str().parse().map_err(|_| ())?;
                }
                Rule::interval_minutes => {
                    time_interval.minutes =
                        rec.as_str().parse().map_err(|_| ())?;
                }
                Rule::interval_seconds => {
                    time_interval.seconds =
                        rec.as_str().parse().map_err(|_| ())?;
                }
                _ => unreachable!(),
            }
        }
        Ok(time_interval)
    }
}

impl Parse for DateInterval {
    fn parse(pair: Pair<'_, Rule>) -> Result<Self, ()> {
        let mut date_interval = Self::default();
        for rec in pair.into_inner() {
            match rec.as_rule() {
                Rule::interval_years => {
                    date_interval.years =
                        rec.as_str().parse().map_err(|_| ())?;
                }
                Rule::interval_months => {
                    date_interval.months =
                        rec.as_str().parse().map_err(|_| ())?;
                }
                Rule::interval_weeks => {
                    date_interval.weeks =
                        rec.as_str().parse().map_err(|_| ())?;
                }
                Rule::interval_days => {
                    date_interval.days =
                        rec.as_str().parse().map_err(|_| ())?;
                }
                _ => unreachable!(),
            }
        }
        Ok(date_interval)
    }
}

impl Parse for TimeRange {
    fn parse(pair: Pair<'_, Rule>) -> Result<Self, ()> {
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
        Ok(time_range)
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
    fn parse(pair: Pair<'_, Rule>) -> Result<Self, ()> {
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
        Ok(recurrence)
    }
}

impl Parse for Countdown {
    fn parse(pair: Pair<'_, Rule>) -> Result<Self, ()> {
        let mut countdown = Self::default();
        for rec in pair.into_inner() {
            match rec.as_rule() {
                Rule::interval => {
                    countdown.durations.push(Interval::parse(rec)?);
                }
                _ => unreachable!(),
            }
        }
        Ok(countdown)
    }
}

impl Parse for Description {
    fn parse(pair: Pair<'_, Rule>) -> Result<Self, ()> {
        Ok(Self(pair.as_str().to_string()))
    }
}

impl Parse for Reminder {
    fn parse(pair: Pair<'_, Rule>) -> Result<Self, ()> {
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
                Rule::EOI => {}
                _ => unreachable!(),
            }
        }
        Ok(reminder)
    }
}

pub(crate) fn parse_reminder(s: &str) -> Result<Reminder, ()> {
    Reminder::parse(
        ReminderParser::parse(Rule::reminder, s)
            .map_err(|err| {
                log::debug!("{}", err);
            })?
            .next()
            .ok_or(())?,
    )
}
