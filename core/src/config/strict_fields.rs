//! One JSON object read field by field, as the phone's settings readers do. Every read marks the
//! field as known, and `finish` refuses any field nobody read, so an unknown or misspelt field
//! refuses the file. The sentences are the phone's.
//!
//! Ported from Enforcer's `tools/enforcer-setup/lib/strict-fields.js`.

use crate::models::specs::ReadMode;
use serde_json::{Map, Value};
use std::collections::HashSet;

pub const WHOLE_FILE: &str = "(file)";
pub const MAX_INT: i64 = 2_147_483_647;
pub const WEEKDAYS: [&str; 7] = ["mon", "tue", "wed", "thu", "fri", "sat", "sun"];
pub const MINUTES_PER_DAY: i64 = 1440;

/// Why one settings file cannot be read: the dotted field and a plain reason.
#[derive(Debug, Clone, PartialEq)]
pub struct ConfigProblem {
    pub field: String,
    pub reason: String,
}

impl ConfigProblem {
    pub fn new(field: &str, reason: &str) -> ConfigProblem {
        ConfigProblem { field: field.to_string(), reason: reason.to_string() }
    }

    /// The field in quotes, then the reason; a problem of the whole file is its reason alone.
    pub fn sentence(&self) -> String {
        if self.field == WHOLE_FILE {
            self.reason.clone()
        } else {
            format!("\"{}\" {}", self.field, self.reason)
        }
    }
}

pub type Read<T> = Result<T, ConfigProblem>;

/// Minutes after midnight for "HH:MM" (24:00 is 1440), or `None` when it is not a time.
pub fn parse_minutes(text: &str) -> Option<i64> {
    let b = text.as_bytes();
    let digits = |i: usize| b[i].is_ascii_digit();
    if b.len() != 5 || b[2] != b':' || !(digits(0) && digits(1) && digits(3) && digits(4)) {
        return None;
    }
    let number = |i: usize| i64::from(b[i] - b'0') * 10 + i64::from(b[i + 1] - b'0');
    let (hour, minute) = (number(0), number(3));
    if minute > 59 || hour > 24 || (hour == 24 && minute != 0) {
        return None;
    }
    Some(hour * 60 + minute)
}

/// True when the text `YYYY-MM-DD` (already known to have that shape) names a day that exists.
pub fn is_real_date(text: &str) -> bool {
    let part = |range: std::ops::Range<usize>| text[range].parse::<i64>().unwrap_or(0);
    let (year, month, day) = (part(0..4), part(5..7), part(8..10));
    if !(1..=12).contains(&month) || day < 1 {
        return false;
    }
    let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
    let lengths = [31, if leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    day <= lengths[(month - 1) as usize]
}

/// The length JavaScript gives a string: UTF-16 code units, which is how the phone counts.
pub fn utf16_len(text: &str) -> i64 {
    text.encode_utf16().count() as i64
}

/// A JSON number that is a whole number inside the 32-bit range, as the phone reads one.
pub fn whole_value(value: &Value) -> Option<i64> {
    let number = value.as_f64()?;
    if !number.is_finite() || number.fract() != 0.0 || number < -2_147_483_648.0 || number > 2_147_483_647.0 {
        return None;
    }
    Some(number as i64)
}

pub struct StrictFields<'a> {
    json: &'a Map<String, Value>,
    path: String,
    known: HashSet<String>,
    mode: ReadMode,
}

impl<'a> StrictFields<'a> {
    /// Fields read in the extended mode; `with_mode` picks the phone's own.
    pub fn new(json: &'a Map<String, Value>, path: &str) -> StrictFields<'a> {
        StrictFields { json, path: path.to_string(), known: HashSet::new(), mode: ReadMode::Extended }
    }

    /// The same fields, read in `mode`. Every field read from these inherits it.
    pub fn with_mode(mut self, mode: ReadMode) -> StrictFields<'a> {
        self.mode = mode;
        self
    }

    pub fn mode(&self) -> ReadMode {
        self.mode
    }

    pub fn fail<T>(&self, name: &str, reason: &str) -> Read<T> {
        let joined = self.join(name);
        Err(ConfigProblem::new(if joined.is_empty() { WHOLE_FILE } else { &joined }, reason))
    }

    pub fn join(&self, name: &str) -> String {
        if self.path.is_empty() {
            name.to_string()
        } else if name.is_empty() {
            self.path.clone()
        } else {
            format!("{}.{}", self.path, name)
        }
    }

    pub fn has(&mut self, name: &str) -> bool {
        self.known.insert(name.to_string());
        self.json.contains_key(name)
    }

    pub fn raw(&mut self, name: &str) -> Read<Option<&'a Value>> {
        self.known.insert(name.to_string());
        let json: &'a Map<String, Value> = self.json;
        match json.get(name) {
            None => Ok(None),
            Some(Value::Null) => self.fail(name, "must not be null"),
            Some(value) => Ok(Some(value)),
        }
    }

    /// A text of `min` to `max` characters; `None` when the field is left out.
    pub fn text_opt(&mut self, name: &str, min: i64, max: i64) -> Read<Option<String>> {
        match self.raw(name)? {
            None => Ok(None),
            Some(Value::String(text)) => {
                let length = utf16_len(text);
                if length < min || length > max {
                    let reason = if max == MAX_INT { format!("must be at least {min} characters") } else { format!("must be {min} to {max} characters") };
                    return self.fail(name, &reason);
                }
                Ok(Some(text.clone()))
            }
            Some(_) => self.fail(name, "must be text"),
        }
    }

    pub fn text_len(&mut self, name: &str, min: i64, max: i64) -> Read<String> {
        match self.text_opt(name, min, max)? {
            Some(text) => Ok(text),
            None => self.fail(name, "is required"),
        }
    }

    pub fn text(&mut self, name: &str) -> Read<String> {
        self.text_len(name, 1, MAX_INT)
    }

    pub fn whole_opt(&mut self, name: &str, min: i64, max: i64) -> Read<Option<i64>> {
        match self.raw(name)? {
            None => Ok(None),
            Some(value) => match whole_value(value) {
                Some(number) if number >= min && number <= max => Ok(Some(number)),
                _ => self.fail(name, &format!("must be a whole number from {min} to {max}")),
            },
        }
    }

    pub fn whole(&mut self, name: &str, min: i64, max: i64) -> Read<i64> {
        match self.whole_opt(name, min, max)? {
            Some(number) => Ok(number),
            None => self.fail(name, "is required"),
        }
    }

    /// A whole number, or the word `word`; `None` stands for the word.
    pub fn whole_or_word(&mut self, name: &str, min: i64, max: i64, word: &str) -> Read<Option<i64>> {
        let Some(value) = self.raw(name)? else {
            return self.fail(name, "is required");
        };
        let problem = format!("must be a whole number from {min} to {max} or \"{word}\"");
        if let Value::String(text) = value {
            return if text == word { Ok(None) } else { self.fail(name, &problem) };
        }
        match whole_value(value) {
            Some(number) if number >= min && number <= max => Ok(Some(number)),
            _ => self.fail(name, &problem),
        }
    }

    /// An "HH:MM" time as minutes after midnight; "24:00" only when `allow_end_of_day`.
    pub fn clock_opt(&mut self, name: &str, allow_end_of_day: bool) -> Read<Option<i64>> {
        let Some(text) = self.text_opt(name, 1, MAX_INT)? else {
            return Ok(None);
        };
        match parse_minutes(&text) {
            Some(minutes) if minutes != MINUTES_PER_DAY || allow_end_of_day => Ok(Some(minutes)),
            _ => self.fail(name, &format!("must be a time written HH:MM{}", if allow_end_of_day { " (24:00 allowed)" } else { "" })),
        }
    }

    pub fn clock(&mut self, name: &str, allow_end_of_day: bool) -> Read<i64> {
        match self.clock_opt(name, allow_end_of_day)? {
            Some(minutes) => Ok(minutes),
            None => self.fail(name, "is required"),
        }
    }

    /// A "YYYY-MM-DD" date that exists, as text.
    pub fn date(&mut self, name: &str) -> Read<String> {
        let text = self.text(name)?;
        let b = text.as_bytes();
        let shaped = b.len() == 10 && b[4] == b'-' && b[7] == b'-' && b.iter().enumerate().all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit());
        if !shaped {
            return self.fail(name, "must be a date written YYYY-MM-DD");
        }
        if !is_real_date(&text) {
            return self.fail(name, "must be a date that exists");
        }
        Ok(text)
    }

    /// Weekday names; none named, or all seven, means every day and gives an empty list. A weekday
    /// named twice is refused, as the schema's uniqueItems says.
    pub fn days(&mut self, name: &str) -> Read<Vec<String>> {
        let Some(names) = self.texts(name, 0, MAX_INT, 1, MAX_INT, false)? else {
            return Ok(Vec::new());
        };
        for (index, code) in names.iter().enumerate() {
            if !WEEKDAYS.contains(&code.as_str()) {
                return self.fail(&format!("{name}[{index}]"), "must be mon, tue, wed, thu, fri, sat or sun");
            }
        }
        let distinct: HashSet<&String> = names.iter().collect();
        if distinct.len() != names.len() {
            return self.fail(name, "must not name a weekday twice");
        }
        let unique: Vec<String> = WEEKDAYS.iter().filter(|day| names.iter().any(|n| n == *day)).map(|day| day.to_string()).collect();
        Ok(if unique.len() == WEEKDAYS.len() { Vec::new() } else { unique })
    }

    /// True when `name` holds exactly the number `expected`.
    pub fn is_stamped_with(&mut self, name: &str, expected: i64) -> bool {
        self.known.insert(name.to_string());
        self.json.get(name).and_then(whole_value) == Some(expected)
    }

    pub fn flag(&mut self, name: &str) -> Read<Option<bool>> {
        match self.raw(name)? {
            None => Ok(None),
            Some(Value::Bool(flag)) => Ok(Some(*flag)),
            Some(_) => self.fail(name, "must be true or false"),
        }
    }

    pub fn child(&mut self, name: &str) -> Read<Option<StrictFields<'a>>> {
        match self.raw(name)? {
            None => Ok(None),
            Some(Value::Object(map)) => Ok(Some(StrictFields::new(map, &self.join(name)).with_mode(self.mode))),
            Some(_) => self.fail(name, "must be an object"),
        }
    }

    pub fn child_required(&mut self, name: &str) -> Read<StrictFields<'a>> {
        match self.child(name)? {
            Some(child) => Ok(child),
            None => self.fail(name, "is required"),
        }
    }

    pub fn list(&mut self, name: &str, required: bool) -> Read<Option<&'a Vec<Value>>> {
        match self.raw(name)? {
            None if required => self.fail(name, "is required"),
            None => Ok(None),
            Some(Value::Array(items)) => Ok(Some(items)),
            Some(_) => self.fail(name, "must be a list"),
        }
    }

    /// A list of texts, `count_min` to `count_max` of them, each of `min` to `max` characters.
    pub fn texts(&mut self, name: &str, count_min: i64, count_max: i64, min: i64, max: i64, required: bool) -> Read<Option<Vec<String>>> {
        let Some(array) = self.list(name, required)? else {
            return Ok(None);
        };
        self.check_count(name, array.len(), count_min, count_max)?;
        let mut found = Vec::with_capacity(array.len());
        for (index, value) in array.iter().enumerate() {
            let Value::String(text) = value else {
                return self.fail(&format!("{name}[{index}]"), "must be text");
            };
            let length = utf16_len(text);
            if length < min || length > max {
                return self.fail(&format!("{name}[{index}]"), &format!("must be {min} to {max} characters"));
            }
            found.push(text.clone());
        }
        Ok(Some(found))
    }

    /// The objects of a list, each as strict fields at the path `name[index]`.
    pub fn objects(&mut self, name: &str, count_min: i64, count_max: i64, required: bool) -> Read<Option<Vec<StrictFields<'a>>>> {
        let Some(array) = self.list(name, required)? else {
            return Ok(None);
        };
        self.check_count(name, array.len(), count_min, count_max)?;
        let mut found = Vec::with_capacity(array.len());
        for (index, value) in array.iter().enumerate() {
            let Value::Object(map) = value else {
                return self.fail(&format!("{name}[{index}]"), "must be an object");
            };
            found.push(StrictFields::new(map, &format!("{}[{}]", self.join(name), index)).with_mode(self.mode));
        }
        Ok(Some(found))
    }

    fn check_count(&self, name: &str, size: usize, min: i64, max: i64) -> Read<()> {
        let size = size as i64;
        if size >= min && size <= max {
            return Ok(());
        }
        let reason = if max == MAX_INT { format!("must hold at least {min} entries") } else { format!("must hold {min} to {max} entries") };
        self.fail(name, &reason)
    }

    /// Refuses every field that no read asked for.
    pub fn finish(&self) -> Read<()> {
        match self.json.keys().find(|key| !self.known.contains(*key)) {
            Some(extra) => self.fail(extra, "is not a field of this file"),
            None => Ok(()),
        }
    }
}
