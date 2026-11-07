//! The one moment form the additions use: UTC, written YYYY-MM-DDTHH:MM:SSZ with a four-digit year,
//! a real date, an hour up to 23, minutes and seconds up to 59 (no leap second, no offset, no zone
//! name). Session ends and moment seals are written this way, so a zone or a clock change never
//! moves them. The text has a fixed width, so the order of two moments is the order of their texts.
//! This module never reads the clock or the time zone: it only counts days of the calendar.

pub const MOMENT_LENGTH: usize = 20;
const LAST_YEAR: i64 = 9999;
const DAY_SECONDS: i64 = 86_400;
const SECOND_MS: i64 = 1_000;
/// Days from 0000-03-01 to 1970-01-01, the shift between the calendar count and the epoch.
const EPOCH_SHIFT: i64 = 719_468;
const DAYS_PER_400_YEARS: i64 = 146_097;

fn number(digits: &[u8]) -> Option<i64> {
    if !digits.iter().all(u8::is_ascii_digit) {
        return None;
    }
    Some(digits.iter().fold(0, |sum, digit| sum * 10 + i64::from(digit - b'0')))
}

fn is_leap(year: i64) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}

fn month_days(year: i64, month: i64) -> i64 {
    match month {
        2 if is_leap(year) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// Days since 1970-01-01 of a real calendar date (months from March, so the leap day ends the year).
fn days_from_date(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let month_from_march = if month > 2 { month - 3 } else { month + 9 };
    let day_of_year = (153 * month_from_march + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * DAYS_PER_400_YEARS + day_of_era - EPOCH_SHIFT
}

/// The (year, month, day) of a count of days since 1970-01-01.
fn date_from_days(days: i64) -> (i64, i64, i64) {
    let shifted = days + EPOCH_SHIFT;
    let era = shifted.div_euclid(DAYS_PER_400_YEARS);
    let day_of_era = shifted - era * DAYS_PER_400_YEARS;
    let year_of_era = (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_from_march = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_from_march + 2) / 5 + 1;
    let month = if month_from_march < 10 { month_from_march + 3 } else { month_from_march - 9 };
    let year = year_of_era + era * 400;
    (if month <= 2 { year + 1 } else { year }, month, day)
}

/// The instant, in milliseconds since the epoch, that `text` names; `None` when `text` is not a
/// moment in the form above.
pub fn parse_utc_moment(text: &str) -> Option<i64> {
    let bytes = text.as_bytes();
    let signs = [(4, b'-'), (7, b'-'), (10, b'T'), (13, b':'), (16, b':'), (19, b'Z')];
    if bytes.len() != MOMENT_LENGTH || signs.iter().any(|&(at, sign)| bytes[at] != sign) {
        return None;
    }
    let (year, month, day) = (number(&bytes[0..4])?, number(&bytes[5..7])?, number(&bytes[8..10])?);
    let (hour, minute, second) = (number(&bytes[11..13])?, number(&bytes[14..16])?, number(&bytes[17..19])?);
    if !(1..=12).contains(&month) || day < 1 || day > month_days(year, month) || hour > 23 || minute > 59 || second > 59 {
        return None;
    }
    let seconds = days_from_date(year, month, day) * DAY_SECONDS + hour * 3600 + minute * 60 + second;
    Some(seconds * SECOND_MS)
}

/// The moment text of a whole-second instant; a part of a second is cut off. `None` when the
/// instant is outside the years 0 to 9999, which the text cannot hold.
pub fn format_utc_moment(instant: i64) -> Option<String> {
    let seconds = instant.div_euclid(SECOND_MS);
    let (year, month, day) = date_from_days(seconds.div_euclid(DAY_SECONDS));
    if !(0..=LAST_YEAR).contains(&year) {
        return None;
    }
    let of_day = seconds.rem_euclid(DAY_SECONDS);
    Some(format!("{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z", of_day / 3600, of_day % 3600 / 60, of_day % 60))
}
