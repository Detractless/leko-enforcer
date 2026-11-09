//! Helpers shared by the tests that scan source files for what may not be mentioned there: a clock,
//! the environment, a time zone database. Hidden from the library's public API.
#![doc(hidden)]

use std::path::{Path, PathBuf};

/// The forbidden words the source scans look for everywhere but in the allowed module: a clock, the
/// environment a zone or a name is taken from, or a time library. The command-line scan adds the
/// process-exit words on top of this list.
pub const CLOCK_SCAN_WORDS: [&str; 14] = [
    "SystemTime",
    "Instant",
    "UNIX_EPOCH",
    "::now(",
    "Utc::",
    "Local::",
    "chrono::",
    "chrono_tz",
    "iana_time_zone",
    "std::env",
    "env::var",
    "libc",
    "std::time",
    "tzset",
];

/// Every `.rs` file under `folder`, found in a stable order.
pub fn rust_files(folder: &Path, found: &mut Vec<PathBuf>) {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(folder).expect("a source folder").map(|entry| entry.expect("an entry").path()).collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            rust_files(&path, found);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            found.push(path);
        }
    }
}

/// The forbidden words in the code lines of `source` (comments are not code).
pub fn forbidden_reads(source: &str, forbidden: &[&'static str]) -> Vec<&'static str> {
    let code = source.lines().filter(|line| !line.trim_start().starts_with("//"));
    let mut found: Vec<&'static str> = code.flat_map(|line| forbidden.iter().copied().filter(move |word| line.contains(word))).collect();
    found.dedup();
    found
}

/// A block file that covers `covers`, holds `rules`, and has a wall for a door.
pub fn block(id: &str, covers: serde_json::Value, rules: serde_json::Value) -> serde_json::Value {
    serde_json::json!({"schema": 2, "id": id, "name": "B", "covers": covers, "rules": rules, "door": {"kind": "wall"}})
}

/// A block of `block` with more fields written into it (`on`, `message`).
pub fn block_with(id: &str, covers: serde_json::Value, rules: serde_json::Value, more: serde_json::Value) -> serde_json::Value {
    let mut file = block(id, covers, rules);
    for (name, value) in more.as_object().into_iter().flatten() {
        file[name] = value.clone();
    }
    file
}

pub fn always(id: &str, days: &[&str]) -> serde_json::Value {
    serde_json::json!({"id": id, "kind": "always", "days": days})
}

/// An outside-hours rule: `allowed` is a list of (from, to) written HH:MM.
pub fn outside_hours(id: &str, days: &[&str], allowed: &[(&str, &str)]) -> serde_json::Value {
    let stretches: Vec<serde_json::Value> = allowed.iter().map(|(from, to)| serde_json::json!({"from": from, "to": to})).collect();
    serde_json::json!({"id": id, "kind": "outsideHours", "days": days, "allowed": stretches})
}

pub fn session(id: &str, ends_at: &str) -> serde_json::Value {
    serde_json::json!({"id": id, "kind": "session", "endsAt": ends_at})
}

pub fn minutes(id: &str) -> serde_json::Value {
    serde_json::json!({"id": id, "kind": "minutes", "amount": 30, "per": "day", "scope": "block"})
}

pub fn opens(id: &str) -> serde_json::Value {
    serde_json::json!({"id": id, "kind": "opens", "count": 3, "per": "day", "scope": "block"})
}
