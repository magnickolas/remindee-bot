use bitmask_enum::bitmask;

use pest::{iterators::Pair, Parser};

#[derive(Parser)]
#[grammar = "grammars/reminder.pest"]
struct ReminderParser;

#[derive(Debug, Default)]
struct HoleyDate {
    year: Option<u32>,
    month: Option<u32>,
    day: Option<u32>,
}

#[derive(Debug, Default)]
struct Interval {
    years: u32,
    months: u32,
    weeks: u32,
    days: u32,
    hours: u32,
    minutes: u32,
    seconds: u32,
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
enum Weekdays {
    Monday,
    Tuesday,
    Wednesday,
    Thursday,
    Friday,
    Saturday,
    Sunday,
}

#[derive(Debug)]
enum DateDivisor {
    Weekdays(Weekdays),
    Interval(Interval),
}

#[derive(Debug, Default)]
struct DateRange {
    from: Option<HoleyDate>,
    until: Option<HoleyDate>,
    date_divisor: Option<DateDivisor>,
}

#[derive(Debug)]
enum DatePattern {
    Point(HoleyDate),
    Range(DateRange),
}

#[derive(Debug, Default)]
struct Time {
    hour: u32,
    minute: u32,
    second: u32,
}

#[derive(Debug, Default)]
struct TimeInterval {
    hours: u32,
    minutes: u32,
    seconds: u32,
}

#[derive(Debug, Default)]
struct TimeRange {
    from: Option<Time>,
    until: Option<Time>,
    interval: TimeInterval,
}

#[derive(Debug)]
enum TimePattern {
    Point(Time),
    Range(TimeRange),
}

#[derive(Debug, Default)]
struct Recurrence {
    dates_patterns: Vec<DatePattern>,
    time_patterns: Option<Vec<TimePattern>>,
}

#[derive(Debug, Default)]
struct Countdown {
    duration: Interval,
}

#[derive(Debug, Default)]
struct Period {
    duration: Interval,
}

#[derive(Debug)]
enum ReminderPattern {
    Recurrence(Recurrence),
    Countdown(Countdown),
    Period(Period),
}

#[derive(Debug, Default)]
pub struct Reminder {
    description: Option<Description>,
    pattern: Option<ReminderPattern>,
}

#[derive(Debug, Default)]
struct Description(String);

trait Parse {
    fn parse(pair: Pair<'_, Rule>) -> Result<Self, ()>
    where
        Self: Sized;
}

impl Parse for HoleyDate {
    fn parse(pair: Pair<'_, Rule>) -> Result<Self, ()> {
        let mut date = Self::default();
        for rec in pair.into_inner() {
            match rec.as_rule() {
                Rule::year => {
                    date.year = Some(rec.as_str().parse().map_err(|_| ())?);
                }
                Rule::month => {
                    date.month = Some(rec.as_str().parse().map_err(|_| ())?);
                }
                Rule::day => {
                    date.day = Some(rec.as_str().parse().map_err(|_| ())?);
                }
                _ => unreachable!(),
            }
        }
        Ok(date)
    }
}

impl Parse for Interval {
    fn parse(pair: Pair<'_, Rule>) -> Result<Self, ()> {
        let mut int = Self::default();
        for rec in pair.into_inner() {
            match rec.as_rule() {
                Rule::interval_years => {
                    int.years = rec.as_str().parse().map_err(|_| ())?;
                }
                Rule::interval_months => {
                    int.months = rec.as_str().parse().map_err(|_| ())?;
                }
                Rule::interval_weeks => {
                    int.weeks = rec.as_str().parse().map_err(|_| ())?;
                }
                Rule::interval_days => {
                    int.days = rec.as_str().parse().map_err(|_| ())?;
                }
                Rule::interval_hours => {
                    int.hours = rec.as_str().parse().map_err(|_| ())?;
                }
                Rule::interval_minutes => {
                    int.minutes = rec.as_str().parse().map_err(|_| ())?;
                }
                Rule::interval_seconds => {
                    int.seconds = rec.as_str().parse().map_err(|_| ())?;
                }
                _ => unreachable!(),
            }
        }
        Ok(int)
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
        let mut range = Self::default();
        for rec in pair.into_inner() {
            match rec.as_rule() {
                Rule::date_from => {
                    range.from = Some(HoleyDate::parse(rec)?);
                }
                Rule::date_until => {
                    range.until = Some(HoleyDate::parse(rec)?);
                }
                Rule::interval => {
                    range.date_divisor =
                        Some(DateDivisor::Interval(Interval::parse(rec)?));
                }
                Rule::weekdays_range => {
                    if range.date_divisor.is_none() {
                        range.date_divisor =
                            Some(DateDivisor::Weekdays(Weekdays::none()));
                    }
                    if let Some(DateDivisor::Weekdays(ref mut w)) =
                        range.date_divisor
                    {
                        *w |= Weekdays::parse(rec)?;
                    }
                }
                _ => unreachable!(),
            }
        }
        Ok(range)
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
        let mut int = Self::default();
        for rec in pair.into_inner() {
            match rec.as_rule() {
                Rule::interval_hours => {
                    int.hours = rec.as_str().parse().map_err(|_| ())?;
                }
                Rule::interval_minutes => {
                    int.minutes = rec.as_str().parse().map_err(|_| ())?;
                }
                Rule::interval_seconds => {
                    int.seconds = rec.as_str().parse().map_err(|_| ())?;
                }
                _ => unreachable!(),
            }
        }
        Ok(int)
    }
}

impl Parse for TimeRange {
    fn parse(pair: Pair<'_, Rule>) -> Result<Self, ()> {
        let mut range = Self::default();
        for rec in pair.into_inner() {
            match rec.as_rule() {
                Rule::time_from => {
                    range.from = Some(Time::parse(rec)?);
                }
                Rule::time_until => {
                    range.until = Some(Time::parse(rec)?);
                }
                Rule::time_interval => {
                    range.interval = TimeInterval::parse(rec)?;
                }
                _ => unreachable!(),
            }
        }
        Ok(range)
    }
}

impl Recurrence {
    fn extend_time_patterns(&mut self, time_pattern: TimePattern) {
        if self.time_patterns.is_none() {
            self.time_patterns = Some(vec![]);
        }
        self.time_patterns.as_mut().unwrap().push(time_pattern);
    }
}

impl Parse for Recurrence {
    fn parse(pair: Pair<'_, Rule>) -> Result<Self, ()> {
        let mut recur = Self::default();
        for rec in pair.into_inner() {
            match rec.as_rule() {
                Rule::dates_point => {
                    recur
                        .dates_patterns
                        .push(DatePattern::Point(HoleyDate::parse(rec)?));
                }
                Rule::dates_range => {
                    recur
                        .dates_patterns
                        .push(DatePattern::Range(DateRange::parse(rec)?));
                }
                Rule::time_point => {
                    recur.extend_time_patterns(TimePattern::Point(
                        Time::parse(rec)?,
                    ));
                }
                Rule::time_range => {
                    recur.extend_time_patterns(TimePattern::Range(
                        TimeRange::parse(rec)?,
                    ));
                }
                _ => unreachable!(),
            }
        }
        // make sure there's at least one date range
        // the inserted holey range will correspond to the current date point
        if recur.dates_patterns.is_empty() {
            recur
                .dates_patterns
                .push(DatePattern::Point(HoleyDate::default()));
        }
        Ok(recur)
    }
}

impl Parse for Countdown {
    fn parse(pair: Pair<'_, Rule>) -> Result<Self, ()> {
        let mut cd = Self::default();
        for rec in pair.into_inner() {
            match rec.as_rule() {
                Rule::interval => {
                    cd.duration = Interval::parse(rec)?;
                }
                _ => unreachable!(),
            }
        }
        Ok(cd)
    }
}

impl Parse for Period {
    fn parse(pair: Pair<'_, Rule>) -> Result<Self, ()> {
        let mut cd = Self::default();
        for rec in pair.into_inner() {
            match rec.as_rule() {
                Rule::interval => {
                    cd.duration = Interval::parse(rec)?;
                }
                _ => unreachable!(),
            }
        }
        Ok(cd)
    }
}

impl Parse for Description {
    fn parse(pair: Pair<'_, Rule>) -> Result<Self, ()> {
        Ok(Self(pair.as_str().to_string()))
    }
}

impl Parse for Reminder {
    fn parse(pair: Pair<'_, Rule>) -> Result<Self, ()> {
        let mut rem = Self::default();
        for rec in pair.into_inner() {
            match rec.as_rule() {
                Rule::description => {
                    rem.description = Some(Description::parse(rec)?);
                }
                Rule::recurrence => {
                    rem.pattern = Some(ReminderPattern::Recurrence(
                        Recurrence::parse(rec)?,
                    ));
                }
                Rule::countdown => {
                    rem.pattern = Some(ReminderPattern::Countdown(
                        Countdown::parse(rec)?,
                    ));
                }
                Rule::period => {
                    rem.pattern =
                        Some(ReminderPattern::Period(Period::parse(rec)?));
                }
                Rule::EOI => {}
                _ => unreachable!(),
            }
        }
        Ok(rem)
    }
}

pub fn parse_reminder(s: &str) -> Result<Reminder, ()> {
    Reminder::parse(
        ReminderParser::parse(Rule::reminder, s)
            .map_err(|_| ())?
            .next()
            .ok_or(())?,
    )
}
