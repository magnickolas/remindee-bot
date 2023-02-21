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

#[cfg(test)]
mod test {
    use super::*;
    use test_case::test_case;

    fn date(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).unwrap()
    }

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
    struct Date(i32, u32, u32);

    #[test_case(Date(2023, 9, 3) , 1    => Date(2023, 10, 3) ;     "just increment month")]
    #[test_case(Date(2023, 1, 31), 1    => Date(2023, 2, 28) ;    "day should be clipped")]
    #[test_case(Date(2023, 12, 31), 1   => Date(2024, 1, 31) ;           "increment year")]
    #[test_case(Date(2023, 5, 15), 9    => Date(2024, 2, 15) ; "increment year and month")]
    #[test_case(Date(2023, 7, 7), 20    => Date(2025, 3, 7)  ;          "add many months")]
    fn test_add_months(time: Date, months: u32) -> Date {
        let (year, month, day) = (time.0, time.1, time.2);
        let res = add_months(&date(year, month, day), months);
        Date(res.year(), res.month(), res.day())
    }
}
