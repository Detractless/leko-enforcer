//! Runs `leko add-block` in this process at a moment the test chooses, and looks inside the settings
//! zips it saves. The settings files are built by `settings_builder`; the zips are read back with the
//! core's reader, not with the command's own code.
#![allow(dead_code)]

use crate::run_capture::{moment, run_os, Outcome};
use leko_core::io::zip_check::{check_zip, ZipVerdict};
use leko_core::open_zip;
use serde_json::Value;
use std::collections::HashSet;
use std::ffi::OsString;
use std::path::Path;

pub const NOW: &str = "2026-11-05T15:00:00Z";
pub const APP: &str = "com.example.app";

/// `leko add-block FILE FLAGS...` at the UTC moment `now`.
pub fn add_at(file: &Path, flags: &[&str], now: &str) -> Outcome {
    let mut args: Vec<OsString> = vec!["add-block".into(), file.as_os_str().to_os_string()];
    args.extend(flags.iter().map(OsString::from));
    run_os(args, moment(now))
}

/// `add_at` at the standard moment.
pub fn add(file: &Path, flags: &[&str]) -> Outcome {
    add_at(file, flags, NOW)
}

/// The flags of a plain request: an app block named "Social Media" that is closed all day.
pub fn plain() -> Vec<&'static str> {
    vec!["--name", "Social Media", "--app", APP, "--always"]
}

/// `plain` and then `more`.
pub fn plain_and<'a>(more: &[&'a str]) -> Vec<&'a str> {
    let mut flags: Vec<&'a str> = plain();
    flags.extend_from_slice(more);
    flags
}

/// The files of the zip at `path`, sorted by name, as the core's reader returns them.
pub fn entries_of(path: &Path) -> Vec<(String, Vec<u8>)> {
    open_zip(&std::fs::read(path).expect("the settings file is readable")).unwrap_or_else(|sentence| panic!("{}: {sentence}", path.display())).files
}

/// The block file `blocks/ID.json` of the zip at `path`, as JSON, or none when the zip has none.
pub fn saved_block(path: &Path, id: &str) -> Option<Value> {
    let wanted = format!("blocks/{id}.json");
    entries_of(path).into_iter().find(|(name, _)| *name == wanted).map(|(_, bytes)| serde_json::from_slice(&bytes).expect("a saved block is JSON"))
}

/// Panics unless the core reads the zip at `path` (the extended reading).
pub fn assert_core_reads(path: &Path) {
    match check_zip(&std::fs::read(path).expect("readable"), &HashSet::new()) {
        ZipVerdict::Read(_) => {}
        ZipVerdict::Refused(refusal) => panic!("the core refuses {}: {}", path.display(), refusal.sentence),
    }
}
