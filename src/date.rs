fn is_leap_year(year: i32) -> bool {
    year % 4 == 0 && (year % 400 == 0 || year % 100 != 0)
}

pub fn days_in_month(month: u32, year: i32) -> i64 {
    match (month, is_leap_year(year)) {
        (2, true) => 29,
        (2, false) => 28,
        _ => {
            if [4, 6, 9, 11].contains(&month) {
                30
            } else {
                31
            }
        }
    }
}

pub fn days_in_year(year: i32) -> i64 {
    if is_leap_year(year) {
        366
    } else {
        365
    }
}
