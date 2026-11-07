//! Reads the words after `leko add-block` into one request: the settings file, the name, the apps and
//! sites, exactly one rule, the days, the door, the message and the write switch. Nothing is guessed:
//! a missing, repeated, unknown or odd argument is a one-sentence problem and no request. Only the
//! shape of the words is judged here (a day name, HH:MM-HH:MM, a length, a whole number); whether a
//! value is good enough to be saved (a wait of 900 seconds, an app name with a space) is the core's
//! judgment. This file opens no file and reads no clock.

use crate::session::session_length::{count_of, read_length};
use crate::session::session_report::shown;
use leko_core::blocks::block_request::{BlockRequest, DoorChoice, RuleChoice};
use leko_core::config::strict_fields::{parse_minutes, WEEKDAYS};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddBlockRequest {
    /// The settings zip, as typed.
    pub file: String,
    pub block: BlockRequest,
    /// True when the block is to be saved, false when only the draft is shown.
    pub write: bool,
}

/// The word after the flag at `words[index]`, which must not look like another flag.
fn value_after<'a>(words: &[&'a str], index: usize, flag: &str, what: &str) -> Result<&'a str, String> {
    match words.get(index + 1) {
        Some(value) if !value.starts_with('-') => Ok(value),
        _ => Err(format!("{flag} needs {what} after it.")),
    }
}

fn once<T>(slot: &mut Option<T>, flag: &str, value: T) -> Result<(), String> {
    if slot.is_some() {
        return Err(format!("{flag} was given twice."));
    }
    *slot = Some(value);
    Ok(())
}

fn switch(slot: &mut bool, flag: &str) -> Result<(), String> {
    if *slot {
        return Err(format!("{flag} was given twice."));
    }
    *slot = true;
    Ok(())
}

fn days_of(text: &str) -> Result<Vec<String>, String> {
    let mut days: Vec<String> = Vec::new();
    for day in text.split(',') {
        if !WEEKDAYS.contains(&day) {
            return Err(format!("{} is not a weekday: write mon, tue, wed, thu, fri, sat or sun, separated by commas.", shown(day)));
        }
        if days.iter().any(|named| named == day) {
            return Err(format!("The weekday {day} is named twice."));
        }
        days.push(day.to_string());
    }
    Ok(days)
}

fn hours_of(text: &str) -> Result<Vec<(String, String)>, String> {
    let mut stretches = Vec::new();
    for stretch in text.split(',') {
        match stretch.split_once('-') {
            Some((from, to)) if parse_minutes(from).is_some() && parse_minutes(to).is_some() => stretches.push((from.to_string(), to.to_string())),
            _ => return Err(format!("{} is not a stretch of hours: write HH:MM-HH:MM, such as 09:00-17:00 (more than one: 09:00-12:00,19:00-22:00).", shown(stretch))),
        }
    }
    Ok(stretches)
}

fn seconds_of(text: &str) -> Result<i64, String> {
    count_of(text).ok_or_else(|| format!("{} is not a number of seconds: write whole seconds, such as 30.", shown(text)))
}

fn minutes_of(text: &str) -> Result<i64, String> {
    read_length(text).ok_or_else(|| format!("{} is not a length: write whole minutes, or a whole number with m, h or d (90, 90m, 2h, 7d).", shown(text)))
}

/// Every flag of the command and what it was given; `check` turns them into a request.
#[derive(Default)]
struct Found<'a> {
    name: Option<&'a str>,
    apps: Vec<&'a str>,
    sites: Vec<&'a str>,
    always: bool,
    hours: Option<Vec<(String, String)>>,
    length: Option<i64>,
    days: Option<Vec<String>>,
    wall: bool,
    wait: Option<i64>,
    message: Option<&'a str>,
    write: bool,
    files: Vec<&'a str>,
}

/// The flags that stand alone; every other flag takes the word after it.
const SWITCHES: [&str; 3] = ["--always", "--wall", "--write"];

fn add_once<'a>(list: &mut Vec<&'a str>, flag: &str, text: &'a str) -> Result<(), String> {
    if list.contains(&text) {
        return Err(format!("{flag} {} was given twice.", shown(text)));
    }
    list.push(text);
    Ok(())
}

fn collect<'a>(words: &[&'a str]) -> Result<Found<'a>, String> {
    let mut found = Found::default();
    let mut index = 0;
    while index < words.len() {
        let flag = words[index];
        let value = |what: &str| value_after(words, index, flag, what);
        match flag {
            "--name" => once(&mut found.name, flag, value("a name")?)?,
            "--app" => add_once(&mut found.apps, flag, value("an app name")?)?,
            "--site" => add_once(&mut found.sites, flag, value("a website host")?)?,
            "--open-hours" => once(&mut found.hours, flag, hours_of(value("hours such as 09:00-17:00")?)?)?,
            "--for" => once(&mut found.length, flag, minutes_of(value("a length such as 90m")?)?)?,
            "--days" => once(&mut found.days, flag, days_of(value("weekdays such as mon,tue")?)?)?,
            "--wait" => once(&mut found.wait, flag, seconds_of(value("a number of seconds")?)?)?,
            "--message" => once(&mut found.message, flag, value("the message")?)?,
            "--always" => switch(&mut found.always, flag)?,
            "--wall" => switch(&mut found.wall, flag)?,
            "--write" => switch(&mut found.write, flag)?,
            other if other.starts_with('-') => return Err(format!("{} is not a flag of this command.", shown(other))),
            file => found.files.push(file),
        }
        index += if flag.starts_with('-') && !SWITCHES.contains(&flag) { 2 } else { 1 };
    }
    Ok(found)
}

fn check(found: Found) -> Result<AddBlockRequest, String> {
    let file = match found.files.as_slice() {
        [file] => (*file).to_string(),
        [] => return Err("No settings file was given.".to_string()),
        _ => return Err("Only one settings file can be given.".to_string()),
    };
    let name = found.name.ok_or("Say what to call the block: --name NAME.")?;
    if found.apps.is_empty() && found.sites.is_empty() {
        return Err("Say what to close: --app ID or --site HOST (more than one is fine).".to_string());
    }
    let rule = match (found.always, found.hours, found.length) {
        (true, None, None) => RuleChoice::Always,
        (false, Some(stretches), None) => RuleChoice::OpenHours(stretches),
        (false, None, Some(minutes)) => RuleChoice::ForMinutes(minutes),
        (false, None, None) => return Err("Say when the block closes: --always, --open-hours HH:MM-HH:MM or --for LENGTH.".to_string()),
        _ => return Err("Give only one of --always, --open-hours and --for.".to_string()),
    };
    if found.days.is_some() && matches!(rule, RuleChoice::ForMinutes(_)) {
        return Err("--days cannot be used with --for: a timed block ends when its time is up.".to_string());
    }
    let door = match (found.wall, found.wait) {
        (true, Some(_)) => return Err("Give either --wall or --wait, not both.".to_string()),
        (true, None) => DoorChoice::Wall,
        (false, Some(seconds)) => DoorChoice::Wait(seconds),
        (false, None) => DoorChoice::Default,
    };
    let block = BlockRequest {
        name: name.to_string(),
        apps: found.apps.iter().map(|app| (*app).to_string()).collect(),
        sites: found.sites.iter().map(|site| (*site).to_string()).collect(),
        rule,
        days: found.days.unwrap_or_default(),
        door,
        message: found.message.map(str::to_string),
    };
    Ok(AddBlockRequest { file, block, write: found.write })
}

/// The request in `words` (the program name and the word `add-block` already cut off), or the
/// problem with them in one sentence.
pub fn read_add_block_arguments(words: &[String]) -> Result<AddBlockRequest, String> {
    let words: Vec<&str> = words.iter().map(String::as_str).collect();
    check(collect(&words)?)
}
