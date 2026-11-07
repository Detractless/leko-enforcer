//! The typed length of a session: whole minutes ("90") or one whole number with one unit letter
//! ("90m", "2h", "7d"). Counting never overflows: a number too big to count becomes the largest
//! number, which the core refuses by its range, so a long number never crashes the program.

pub const MINUTES_PER_HOUR: i64 = 60;
pub const MINUTES_PER_DAY: i64 = 1_440;

/// `count` units of `unit_minutes` minutes each; the largest number when that is too big to count.
pub fn minutes_of(count: i64, unit_minutes: i64) -> i64 {
    count.saturating_mul(unit_minutes)
}

/// The value of a text made only of ASCII digits (the largest number when it is too big to count);
/// `None` for empty text, a sign, a point, a space or any other character.
pub fn count_of(digits: &str) -> Option<i64> {
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    Some(digits.bytes().fold(0_i64, |sum, digit| sum.saturating_mul(10).saturating_add(i64::from(digit - b'0'))))
}

/// The minutes a typed length names, or `None` when the text is not a length.
pub fn read_length(text: &str) -> Option<i64> {
    let (digits, unit_minutes) = if let Some(rest) = text.strip_suffix('m') {
        (rest, 1)
    } else if let Some(rest) = text.strip_suffix('h') {
        (rest, MINUTES_PER_HOUR)
    } else if let Some(rest) = text.strip_suffix('d') {
        (rest, MINUTES_PER_DAY)
    } else {
        (text, 1)
    };
    count_of(digits).map(|count| minutes_of(count, unit_minutes))
}
