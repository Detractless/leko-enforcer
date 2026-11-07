//! Reads the words a speech-to-text heard into one length in minutes. It takes only what it is sure
//! of: the word "session" and exactly one length, said as digits and a unit, as "a", "an" or "one"
//! and a unit, or as "half an hour" or "half hour". Anything else (no length, two lengths, a number
//! in words, a decimal, a minus, a cancelling or denying word) makes nothing and says why, because a
//! wrong guess would seal the user in for up to 7 days and a seal cannot be loosened early.

use super::session_length::{count_of, minutes_of, MINUTES_PER_DAY, MINUTES_PER_HOUR};

pub const MAX_PHRASE_CHARACTERS: usize = 500;
pub const EXAMPLES: &str = "\"start a 25 minute session\", \"make a session for 2 hours\" or \"a half hour session\"";

const HALF_HOUR_MINUTES: i64 = 30;
const SESSION_WORD: &str = "session";

/// Whole words that cancel, end, undo or deny; one anywhere in the phrase refuses it. Apostrophes
/// are dropped before words are cut, so "don't" is "dont".
const DENYING_WORDS: &[&str] = &[
    "no", "not", "dont", "cant", "wont", "never", "without", "end", "ends", "ended", "ending", "quit", "quits", "quitting", "undo",
];
/// The beginnings of words that cancel, end, undo or deny (cancel, cancelled, removing, ...).
const DENYING_STEMS: &[&str] = &["cancel", "stop", "remov", "delet", "unlock", "shorten", "disabl", "abort", "skip"];
/// Units of time that are not a length this command takes.
const OTHER_UNITS: &[&str] = &[
    "second", "seconds", "sec", "secs", "millisecond", "milliseconds", "week", "weeks", "fortnight", "fortnights", "month", "months", "year", "years", "decade",
    "decades",
];
/// Numbers said in words (only "a", "an" and "one" are taken, as 1).
const NUMBER_WORDS: &[&str] = &[
    "zero", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten", "eleven", "twelve", "thirteen", "fourteen", "fifteen", "sixteen", "seventeen",
    "eighteen", "nineteen", "twenty", "thirty", "forty", "fifty", "sixty", "seventy", "eighty", "ninety", "hundred", "thousand", "million", "couple", "few",
    "several", "dozen", "quarter",
];

fn unit_minutes(word: &str) -> Option<i64> {
    match word {
        "minute" | "minutes" | "min" | "mins" => Some(1),
        "hour" | "hours" | "hr" | "hrs" => Some(MINUTES_PER_HOUR),
        "day" | "days" => Some(MINUTES_PER_DAY),
        _ => None,
    }
}

/// A plural unit cannot follow "a", "an" or "one"; when it does, a number was probably misheard.
fn is_plural_unit(word: &str) -> bool {
    matches!(word, "minutes" | "mins" | "hours" | "hrs" | "days")
}

fn is_denying(word: &str) -> bool {
    DENYING_WORDS.contains(&word) || DENYING_STEMS.iter().any(|stem| word.starts_with(stem))
}

fn is_dash(character: char) -> bool {
    matches!(character, '-' | '\u{2010}'..='\u{2015}' | '\u{2212}')
}

/// The lower-case words of `phrase`, or why it holds a minus, a range or a decimal (a dash, point or
/// comma right before a digit: "-5", "2-3", "1.5", ".5"). A dash joined to a letter ("25-minute")
/// is only a space; apostrophes are dropped; every other mark is a space. Letters and digits of any
/// script stay in their word, so "1٣" is one odd word and not a "1".
fn words_of(phrase: &str) -> Result<Vec<String>, String> {
    let characters: Vec<char> = phrase.chars().collect();
    let digit_after = |at: usize| characters.get(at + 1).is_some_and(|character| character.is_ascii_digit());
    for (at, &character) in characters.iter().enumerate() {
        if is_dash(character) && digit_after(at) {
            return Err("a minus sign or a range was heard".to_string());
        }
        if matches!(character, '.' | ',') && digit_after(at) {
            return Err("a decimal number was heard".to_string());
        }
    }
    let plain: String = characters
        .iter()
        .filter(|character| !matches!(character, '\'' | '\u{2019}'))
        .map(|&character| if character.is_alphanumeric() { character } else { ' ' })
        .collect();
    Ok(plain.to_lowercase().split_whitespace().map(str::to_string).collect())
}

/// What the words at one place are.
enum Piece {
    /// A length of `minutes`, written with `used` words.
    Length { minutes: i64, used: usize },
    /// A word that is no part of a length.
    Skip,
    Refuse(String),
}

fn other_unit(word: &str) -> String {
    format!("{word:?} is not a length this command knows; use minutes, hours or days")
}

fn piece_at(words: &[String], at: usize) -> Piece {
    let word = words[at].as_str();
    let next = words.get(at + 1).map(String::as_str);
    if word == "half" {
        return match (next, words.get(at + 2).map(String::as_str)) {
            (Some("an"), Some("hour")) => Piece::Length { minutes: HALF_HOUR_MINUTES, used: 3 },
            (Some("hour"), _) => Piece::Length { minutes: HALF_HOUR_MINUTES, used: 2 },
            _ => Piece::Refuse("\"half\" is only understood in \"half an hour\" and \"half hour\"".to_string()),
        };
    }
    if unit_minutes(word).is_some() {
        return Piece::Refuse(format!("{word:?} has no number before it"));
    }
    if OTHER_UNITS.contains(&word) {
        return Piece::Refuse(other_unit(word));
    }
    if NUMBER_WORDS.contains(&word) {
        return Piece::Refuse(format!("a number in words ({word:?}) is not understood; say it in digits"));
    }
    let digits = count_of(word);
    let count = match (digits, word) {
        (Some(count), _) => count,
        (None, "a" | "an" | "one") => 1,
        (None, _) if word.bytes().any(|byte| byte.is_ascii_digit()) => return Piece::Refuse(format!("{word:?} is not a plain number")),
        (None, _) => return Piece::Skip,
    };
    if let Some(unit) = next.and_then(unit_minutes) {
        if count == 0 {
            return Piece::Refuse("zero is not a length".to_string());
        }
        if digits.is_none() && next.is_some_and(is_plural_unit) {
            return Piece::Refuse(format!("{word:?} before {:?} may be a number heard wrongly; say it in digits", next.unwrap_or_default()));
        }
        return Piece::Length { minutes: minutes_of(count, unit), used: 2 };
    }
    match next {
        Some(other) if OTHER_UNITS.contains(&other) => Piece::Refuse(other_unit(other)),
        _ if digits.is_some() => Piece::Refuse(format!("the number {word} has no unit after it")),
        _ => Piece::Skip,
    }
}

/// The minutes the heard `phrase` asks for, or the reason it makes nothing (a short phrase with no
/// full stop, such as "two lengths were heard").
pub fn read_phrase(phrase: &str) -> Result<i64, String> {
    if phrase.chars().nth(MAX_PHRASE_CHARACTERS).is_some() {
        return Err(format!("the words are longer than {MAX_PHRASE_CHARACTERS} characters"));
    }
    let words = words_of(phrase)?;
    if words.is_empty() {
        return Err("no words were heard".to_string());
    }
    if let Some(word) = words.iter().find(|word| is_denying(word)) {
        return Err(format!("{word:?} can mean cancel or no"));
    }
    if !words.iter().any(|word| word == SESSION_WORD) {
        return Err(format!("the word \"{SESSION_WORD}\" was not heard"));
    }
    let (mut found, mut at) = (None, 0);
    while at < words.len() {
        match piece_at(&words, at) {
            Piece::Length { minutes, used } => {
                if found.is_some() {
                    return Err("two lengths were heard".to_string());
                }
                found = Some(minutes);
                at += used;
            }
            Piece::Skip => at += 1,
            Piece::Refuse(reason) => return Err(reason),
        }
    }
    found.ok_or_else(|| "no length was heard".to_string())
}
