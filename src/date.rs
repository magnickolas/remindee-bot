fn is_leap_year(year: i32) -> bool {
    return year % 4 == 0 && (year % 400 == 0 || year % 100 != 0);
}

pub fn days_in_month(month: u32, year: i32) -> i64 {
    match (month, is_leap_year(year)) {
        (2, true) => 29,
        (2, false) => 28,
        (m, _) if m == 4 || m == 6 || m == 9 || m == 11 => 30,
        _ => 31,
    }
}

pub fn days_in_year(year: i32) -> i64 {
    if is_leap_year(year) {
        366
    } else {
        365
    }
}
