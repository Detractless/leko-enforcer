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
#[allow(unused_imports)]
pub use leko_core::test_support::{always, block, block_with, minutes, opens, outside_hours, session};

use std::collections::HashSet;

pub fn every_open(id: &str, days: &[&str]) -> Value {
    json!({"id": id, "kind": "everyOpen", "days": days})
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

pub const APP: &str = "com.a";
pub const NEW_YORK: &str = "America/New_York";
pub const DAY_MS: i64 = 86_400_000;

/// The settings of one rule of one block that covers APP.
pub fn one_rule(rule: Value) -> ReadSet {
    read_set(&[block("b", json!({"apps": [APP]}), json!([rule]))])
}
