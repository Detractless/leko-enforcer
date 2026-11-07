//! Builds the settings a switch-off test judges: blocks with locks, routines that hold blocks, the
//! practice a routine needs, and the guide and general file every export has, written as a zip the
//! way the phone exports it and read back through the real readers.
#![allow(dead_code)]

use crate::zip_writer::zip_of;
use leko_core::io::zip_check::{check_zip, ZipVerdict};
use leko_core::models::specs::ReadSet;
use serde_json::{json, Value};
use std::collections::HashSet;

pub const KEY: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

/// A block file for the app `com.<id>` with an always-closed rule and the lock `lock` (none when null).
pub fn locked_block(id: &str, lock: Value) -> Value {
    let mut file = json!({"schema": 2, "id": id, "name": "B", "covers": {"apps": [format!("com.{id}")]}, "rules": [{"id": "r", "kind": "always"}], "door": {"kind": "wall"}});
    if !lock.is_null() {
        file["lock"] = lock;
    }
    file
}

/// The same block switched off.
pub fn off_block(id: &str) -> Value {
    let mut file = locked_block(id, Value::Null);
    file["on"] = json!(false);
    file
}

/// A routine that holds `held` when skipped and carries `lock` (none when null).
pub fn routine_holding(id: &str, held: &[&str], lock: Value) -> Value {
    let mut file = json!({"schema": 2, "id": id, "name": "R", "practice": "p", "due": {"times": ["07:00"]}, "ifSkipped": {"holdBlocks": held, "takeOver": true}});
    if !lock.is_null() {
        file["lock"] = lock;
    }
    file
}

pub fn wait(hours: i64) -> Value {
    json!({"level": "wait", "hours": hours})
}

pub fn change_hours(from: &str, to: &str, hours: i64) -> Value {
    json!({"level": "changeHours", "from": from, "to": to, "hours": hours})
}

pub fn sealed_until(date: &str) -> Value {
    json!({"level": "sealedUntil", "until": date})
}

pub fn sealed_at(moment: &str) -> Value {
    json!({"level": "sealedAt", "at": moment})
}

/// A lock of `lock` that also carries a partner's key.
pub fn with_partner_key(mut lock: Value) -> Value {
    lock["partnerKeyHash"] = json!({"alg": "sha256", "hash": KEY});
    lock
}

/// The named files of a settings zip with `blocks` and `routines` in it.
pub fn files_of(blocks: &[Value], routines: &[Value]) -> Vec<(String, String)> {
    let mut files = vec![
        ("GUIDE.md".to_string(), "guide".to_string()),
        ("general.json".to_string(), r#"{"schema": 2}"#.to_string()),
        ("practices/p.json".to_string(), r#"{"schema":2,"id":"p","name":"P","activity":"pushUps","amountPerSet":5}"#.to_string()),
    ];
    files.extend(blocks.iter().map(|file| (format!("blocks/{}.json", file["id"].as_str().unwrap_or("x")), file.to_string())));
    files.extend(routines.iter().map(|file| (format!("routines/{}.json", file["id"].as_str().unwrap_or("x")), file.to_string())));
    files
}

/// The bytes of a settings zip with `blocks` and `routines` in it.
pub fn zip_with(blocks: &[Value], routines: &[Value]) -> Vec<u8> {
    let files = files_of(blocks, routines);
    let entries: Vec<(&str, &str)> = files.iter().map(|(name, text)| (name.as_str(), text.as_str())).collect();
    zip_of(&entries)
}

/// The settings of `blocks` and `routines`, read as the readers read a zip.
pub fn set_with(blocks: &[Value], routines: &[Value]) -> ReadSet {
    match check_zip(&zip_with(blocks, routines), &HashSet::new()) {
        ZipVerdict::Read(read) => read.set,
        ZipVerdict::Refused(refusal) => panic!("the settings were refused: {}", refusal.sentence),
    }
}
