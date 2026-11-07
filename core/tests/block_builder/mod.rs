//! Builds the settings a decision test judges: block files written as JSON, put in a zip the way the
//! phone exports it and read back through the real readers, so a test sees exactly what the readers
//! keep. Moments are written as RFC 3339 text, with an offset when the local time matters.
#![allow(dead_code)]

use crate::zip_writer::zip_of;
use chrono::DateTime;
use leko_core::config_time::zone_time::ZoneTime;
use leko_core::io::zip_check::{check_zip, ZipVerdict};
use leko_core::models::specs::ReadSet;
use serde_json::{json, Value};
use std::collections::HashSet;

/// A block file that covers `covers`, holds `rules`, and has a wall for a door.
pub fn block(id: &str, covers: Value, rules: Value) -> Value {
    json!({"schema": 2, "id": id, "name": "B", "covers": covers, "rules": rules, "door": {"kind": "wall"}})
}

/// A block of `block` with more fields written into it (`on`, `message`).
pub fn block_with(id: &str, covers: Value, rules: Value, more: Value) -> Value {
    let mut file = block(id, covers, rules);
    for (name, value) in more.as_object().into_iter().flatten() {
        file[name] = value.clone();
    }
    file
}

pub fn always(id: &str, days: &[&str]) -> Value {
    json!({"id": id, "kind": "always", "days": days})
}

pub fn every_open(id: &str, days: &[&str]) -> Value {
    json!({"id": id, "kind": "everyOpen", "days": days})
}

/// An outside-hours rule: `allowed` is a list of (from, to) written HH:MM.
pub fn outside_hours(id: &str, days: &[&str], allowed: &[(&str, &str)]) -> Value {
    let stretches: Vec<Value> = allowed.iter().map(|(from, to)| json!({"from": from, "to": to})).collect();
    json!({"id": id, "kind": "outsideHours", "days": days, "allowed": stretches})
}

pub fn session(id: &str, ends_at: &str) -> Value {
    json!({"id": id, "kind": "session", "endsAt": ends_at})
}

pub fn minutes(id: &str) -> Value {
    json!({"id": id, "kind": "minutes", "amount": 30, "per": "day", "scope": "block"})
}

pub fn opens(id: &str) -> Value {
    json!({"id": id, "kind": "opens", "count": 3, "per": "day", "scope": "block"})
}

/// The settings of `blocks`, written in this order, read as the readers read a zip.
pub fn read_set(blocks: &[Value]) -> ReadSet {
    let files: Vec<(String, String)> = blocks.iter().map(|file| (format!("blocks/{}.json", file["id"].as_str().unwrap_or("x")), file.to_string())).collect();
    let mut entries: Vec<(&str, &str)> = vec![("GUIDE.md", "guide")];
    entries.extend(files.iter().map(|(path, text)| (path.as_str(), text.as_str())));
    match check_zip(&zip_of(&entries), &HashSet::new()) {
        ZipVerdict::Read(read) => read.set,
        ZipVerdict::Refused(refusal) => panic!("the settings were refused: {}", refusal.sentence),
    }
}

/// An instant written as RFC 3339, in milliseconds since the epoch.
pub fn moment(text: &str) -> i64 {
    DateTime::parse_from_rfc3339(text).unwrap_or_else(|_| panic!("{text} is not a time")).timestamp_millis()
}

pub fn zone(name: &str) -> ZoneTime {
    ZoneTime::new(name).unwrap_or_else(|sentence| panic!("{sentence}"))
}

pub fn none_blocked() -> HashSet<String> {
    HashSet::new()
}
