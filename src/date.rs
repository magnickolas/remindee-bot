use std::cmp::min;

use chrono::{Datelike, NaiveDate};

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

pub fn add_months(date: &NaiveDate, months: u32) -> NaiveDate {
    let total_month = date.month() + months;
    let year = date.year() + total_month as i32 / 12;
    let month = total_month % 12;
    let day = min(date.day(), days_in_month(month, year));
    NaiveDate::from_ymd_opt(year, month, day).unwrap()
}
