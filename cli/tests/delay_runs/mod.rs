//! Runs `leko switch-off`, `leko settle` and `leko cancel` in this process at a moment the test chooses, builds the
//! settings zips and waiting files they work on, and reads the results back with the core's reader.
//! Needs `delay_sets` and `zip_writer` (the core tests' own, shared by path), `run_capture` and
//! `scratch_folder` declared beside it.
#![allow(dead_code)]

use crate::delay_sets::{set_with, zip_with};
use crate::run_capture::{moment, run_in_zone, Outcome};
use leko_cli::add_block::file_replace::{RealSteps, Steps};
use leko_cli::delayed_changes::{run_settle::run_settle_with, run_switch_off::run_switch_off_with};
use leko_cli::waiting_changes::run_cancel::run_cancel_with;
use leko_cli::switch_on::run_switch_on::run_switch_on_with;
use leko_core::io::zip_check::{check_zip, ZipVerdict};
use leko_core::open_zip;
use serde_json::{json, Value};
use std::collections::HashSet;
use std::ffi::OsString;
use std::fs::File;
use std::io::{Error, ErrorKind, Write};
use std::path::{Path, PathBuf};

/// Monday 2 November 2026, 10:00 in New York (the clocks went back on the 1st).
pub const NOW: &str = "2026-11-02T15:00:00Z";
pub const ZONE: &str = "America/New_York";

fn run_command_ms(word: &str, file: &Path, flags: &[&str], now_ms: i64, zone: Option<&str>) -> Outcome {
    let mut args: Vec<OsString> = vec![word.into(), file.as_os_str().to_os_string()];
    args.extend(flags.iter().map(OsString::from));
    run_in_zone(args, now_ms, zone)
}

fn run_command(word: &str, file: &Path, flags: &[&str], now: &str, zone: Option<&str>) -> Outcome {
    run_command_ms(word, file, flags, moment(now), zone)
}

/// `leko switch-off` at the moment `now_ms`, which may hold a part of a second.
pub fn switch_off_ms(file: &Path, flags: &[&str], now_ms: i64) -> Outcome {
    run_command_ms("switch-off", file, flags, now_ms, Some(ZONE))
}

/// `leko switch-off FILE FLAGS...` at the UTC moment `now` on a computer whose zone is New York.
pub fn switch_off(file: &Path, flags: &[&str], now: &str) -> Outcome {
    run_command("switch-off", file, flags, now, Some(ZONE))
}

/// `leko settle FILE FLAGS...` at the UTC moment `now` on a computer whose zone is New York.
pub fn settle(file: &Path, flags: &[&str], now: &str) -> Outcome {
    run_command("settle", file, flags, now, Some(ZONE))
}

/// The same with the computer's zone chosen by the test.
pub fn switch_off_in(file: &Path, flags: &[&str], now: &str, zone: Option<&str>) -> Outcome {
    run_command("switch-off", file, flags, now, zone)
}

pub fn settle_in(file: &Path, flags: &[&str], now: &str, zone: Option<&str>) -> Outcome {
    run_command("settle", file, flags, now, zone)
}

/// Writes a settings zip of `blocks` and `routines` as `name` in `folder`; gives its path.
pub fn write_zip(folder: &Path, name: &str, blocks: &[Value], routines: &[Value]) -> PathBuf {
    let path = folder.join(name);
    std::fs::write(&path, zip_with(blocks, routines)).expect("a settings file");
    path
}

/// The waiting file beside the settings file.
pub fn waiting_of(file: &Path) -> PathBuf {
    PathBuf::from(format!("{}.waiting.json", file.display()))
}

/// One waiting record: the block, the moment it was asked for and the moment it may land.
pub fn record(block: &str, requested_at: &str, due_at: &str) -> Value {
    json!({"kind": "switch-off", "block": block, "requestedAt": requested_at, "dueAt": due_at})
}

/// Writes the waiting file beside `file` with `records`, in the form this version writes.
pub fn write_waiting(file: &Path, records: &[Value]) {
    let text = serde_json::to_string_pretty(&json!({"format": 1, "changes": records})).expect("JSON") + "\n";
    std::fs::write(waiting_of(file), text).expect("a waiting file");
}

/// The records of the waiting file beside `file`.
pub fn records_of(file: &Path) -> Vec<Value> {
    let json: Value = serde_json::from_slice(&std::fs::read(waiting_of(file)).expect("a waiting file")).expect("JSON");
    assert_eq!(json["format"], 1);
    json["changes"].as_array().expect("a list").clone()
}

/// The files of the zip at `path`, sorted by name, as the core's reader returns them.
pub fn entries_of(path: &Path) -> Vec<(String, Vec<u8>)> {
    open_zip(&std::fs::read(path).expect("the settings file is readable")).unwrap_or_else(|sentence| panic!("{}: {sentence}", path.display())).files
}

/// The block file `blocks/ID.json` of the zip at `path`, as JSON.
pub fn block_of(path: &Path, id: &str) -> Value {
    let wanted = format!("blocks/{id}.json");
    let (_, bytes) = entries_of(path).into_iter().find(|(name, _)| *name == wanted).unwrap_or_else(|| panic!("no {wanted}"));
    serde_json::from_slice(&bytes).expect("a block is JSON")
}

/// Panics unless the core reads the zip at `path`.
pub fn assert_core_reads(path: &Path) {
    match check_zip(&std::fs::read(path).expect("readable"), &HashSet::new()) {
        ZipVerdict::Read(_) => {}
        ZipVerdict::Refused(refusal) => panic!("the core refuses {}: {}", path.display(), refusal.sentence),
    }
}

/// Panics unless `block` is a switched-off copy of `before`: the same JSON with `"on": false`.
pub fn assert_switched_off(before: &Value, after: &Value) {
    let mut expected = before.clone();
    expected["on"] = json!(false);
    assert_eq!(*after, expected);
}

/// Every file of the zip at `path` except `blocks/ID.json` for each id in `changed`, as the reader returns them.
pub fn entries_except(path: &Path, changed: &[&str]) -> Vec<(String, Vec<u8>)> {
    let skipped: Vec<String> = changed.iter().map(|id| format!("blocks/{id}.json")).collect();
    entries_of(path).into_iter().filter(|(name, _)| !skipped.contains(name)).collect()
}

/// The settings as the core reads them from a built zip (for tests that need the plan too).
pub fn set_of(blocks: &[Value], routines: &[Value]) -> leko_core::models::specs::ReadSet {
    set_with(blocks, routines)
}

/// The one line `err` is: it ends with a newline and holds no other.
pub fn one_line(err: &str) -> &str {
    assert!(err.ends_with('\n') && err.matches('\n').count() == 1, "one line: {err:?}");
    err
}

/// Every file in `folder` with its bytes, sorted by name: the folder as a test left it.
pub fn snapshot(folder: &Path) -> Vec<(String, Vec<u8>)> {
    let mut files: Vec<(String, Vec<u8>)> = std::fs::read_dir(folder)
        .expect("readable")
        .map(|entry| {
            let entry = entry.expect("an entry");
            (entry.file_name().to_string_lossy().into_owned(), std::fs::read(entry.path()).unwrap_or_default())
        })
        .collect();
    files.sort();
    files
}

/// A step of a save that a test can make fail.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Write,
    Flush,
    Rename,
}

impl Step {
    /// What the program says could not be done when this step fails.
    pub fn told_as(self) -> &'static str {
        match self {
            Step::Write => "the new file could not be written",
            Step::Flush => "the new file could not be flushed to the disk",
            Step::Rename => "the new file could not be moved into place",
        }
    }

    /// The problem the failing step reports.
    pub fn problem(self) -> &'static str {
        match self {
            Step::Write => "disk full",
            Step::Flush => "flush refused",
            Step::Rename => "move refused",
        }
    }
}

/// The real steps of a save, except that the `nth` call (counting from 1) of `step` fails; a failing
/// write puts half of the bytes down first, as a full disk does. `moved_to` lists the files that were
/// moved into place, in order, by the name each was given.
pub struct FailAt {
    failing: Option<(Step, usize)>,
    calls: [usize; 3],
    pub moved_to: Vec<String>,
}

impl FailAt {
    pub fn new(step: Step, nth: usize) -> FailAt {
        FailAt { failing: Some((step, nth)), calls: [0; 3], moved_to: Vec::new() }
    }

    /// Steps that never fail and only record what was moved.
    pub fn recording() -> FailAt {
        FailAt { failing: None, calls: [0; 3], moved_to: Vec::new() }
    }

    fn fails(&mut self, step: Step) -> bool {
        self.calls[step as usize] += 1;
        self.failing == Some((step, self.calls[step as usize]))
    }
}

impl Steps for FailAt {
    fn write(&mut self, file: &mut File, bytes: &[u8]) -> std::io::Result<()> {
        if self.fails(Step::Write) {
            file.write_all(&bytes[..bytes.len() / 2])?;
            return Err(Error::new(ErrorKind::Other, Step::Write.problem()));
        }
        RealSteps.write(file, bytes)
    }

    fn flush(&mut self, file: &File) -> std::io::Result<()> {
        if self.fails(Step::Flush) {
            return Err(Error::new(ErrorKind::Other, Step::Flush.problem()));
        }
        RealSteps.flush(file)
    }

    fn rename(&mut self, from: &Path, to: &Path) -> std::io::Result<()> {
        if self.fails(Step::Rename) {
            return Err(Error::new(ErrorKind::Other, Step::Rename.problem()));
        }
        self.moved_to.push(to.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default());
        RealSteps.rename(from, to)
    }
}

fn words_of(file: &Path, flags: &[&str]) -> Vec<String> {
    std::iter::once(file.to_string_lossy().into_owned()).chain(flags.iter().map(|flag| (*flag).to_string())).collect()
}

fn outcome_of(code: i32, out: Vec<u8>, err: Vec<u8>) -> Outcome {
    Outcome { code, out: String::from_utf8(out).expect("stdout is text"), err: String::from_utf8(err).expect("stderr is text") }
}

/// `leko switch-off FILE FLAGS...` at the UTC moment `now` (New York), saving through `steps`.
pub fn switch_off_with(file: &Path, flags: &[&str], now: &str, steps: &mut dyn Steps) -> Outcome {
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = run_switch_off_with(&words_of(file, flags), moment(now), Some(ZONE), steps, &mut out, &mut err);
    outcome_of(code, out, err)
}

/// `leko settle FILE FLAGS...` at the UTC moment `now` (New York), saving through `steps`.
pub fn settle_with(file: &Path, flags: &[&str], now: &str, steps: &mut dyn Steps) -> Outcome {
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = run_settle_with(&words_of(file, flags), moment(now), Some(ZONE), steps, &mut out, &mut err);
    outcome_of(code, out, err)
}

/// `leko cancel FILE FLAGS...` (it takes no moment and no zone).
pub fn cancel(file: &Path, flags: &[&str]) -> Outcome {
    run_command("cancel", file, flags, NOW, Some(ZONE))
}

/// `leko cancel FILE FLAGS...`, saving through `steps`.
pub fn cancel_with(file: &Path, flags: &[&str], steps: &mut dyn Steps) -> Outcome {
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = run_cancel_with(&words_of(file, flags), steps, &mut out, &mut err);
    outcome_of(code, out, err)
}

/// `leko switch-on FILE FLAGS...` (it takes no moment and no zone).
pub fn switch_on(file: &Path, flags: &[&str]) -> Outcome {
    run_command("switch-on", file, flags, NOW, Some(ZONE))
}

/// `leko switch-on FILE FLAGS...`, saving through `steps`.
pub fn switch_on_with(file: &Path, flags: &[&str], steps: &mut dyn Steps) -> Outcome {
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = run_switch_on_with(&words_of(file, flags), steps, &mut out, &mut err);
    outcome_of(code, out, err)
}

/// Panics unless `after` is a switched-on copy of `before`: the same JSON with `"on": true`.
pub fn assert_switched_on(before: &Value, after: &Value) {
    let mut expected = before.clone();
    expected["on"] = json!(true);
    assert_eq!(*after, expected);
}

/// A block file for the app `com.<id>` that also covers the sites `sites` (none: the app only).
pub fn covering(id: &str, lock: Value, sites: &[&str]) -> Value {
    let mut file = crate::delay_sets::locked_block(id, lock);
    if !sites.is_empty() {
        file["covers"]["sites"] = json!(sites);
    }
    file
}

/// The blocks the status tests ask about: `a` and `s` cover `example.com` (and `s` also `other.org`),
/// `c` only its app, `k` holds a partner's key, and `o` is already off.
pub fn status_blocks() -> Vec<Value> {
    use crate::delay_sets::{off_block, wait, with_partner_key};
    vec![
        covering("a", wait(24), &["example.com"]),
        covering("s", wait(48), &["example.com", "other.org"]),
        covering("c", wait(1), &[]),
        covering("k", with_partner_key(wait(24)), &["keyed.net"]),
        off_block("o"),
    ]
}

/// The parked changes for those blocks, in file order, asked for at 12:00 or 13:00 UTC on 2 November;
/// `gone` has no block in the settings.
pub fn status_records() -> Vec<Value> {
    vec![
        record("c", "2026-11-02T12:00:00Z", "2026-11-02T13:00:00Z"),
        record("s", "2026-11-02T13:00:00Z", "2026-11-04T13:00:00Z"),
        record("gone", "2026-11-02T12:00:00Z", "2026-11-03T12:00:00Z"),
        record("a", "2026-11-02T12:00:00Z", "2026-11-03T12:00:00Z"),
        record("k", "2026-11-02T12:00:00Z", "2026-11-03T12:00:00Z"),
        record("o", "2026-11-02T12:00:00Z", "2026-11-03T12:00:00Z"),
    ]
}

/// Run `command` on `file` for each `(words, code)` case with a disk that fails at the first write,
/// and assert the code and that the folder holds what it held before.
pub fn assert_failed_write_changes_nothing(
    command: fn(&Path, &[&str], &mut dyn Steps) -> Outcome,
    file: &Path,
    folder: &Path,
    before: &[(String, Vec<u8>)],
    cases: &[(&[&str], i32)],
) {
    for (flags, code) in cases {
        let run = command(file, flags, &mut FailAt::new(Step::Write, 1));
        assert_eq!(run.code, *code, "{flags:?}: {}", run.err);
        assert_eq!(snapshot(folder), before, "{flags:?}");
    }
}
