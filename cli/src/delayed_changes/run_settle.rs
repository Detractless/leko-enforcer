//! Runs one `leko settle` request in order: the words (exit 2), the zone, the settings file and the
//! waiting file (exit 1 when either is bad; no waiting file, or an empty list, means nothing is
//! waiting and no file is made), then the core settles every parked change at the given moment. The
//! result is a draft; with `--write` the settings zip is saved first and the shortened waiting file
//! second, so a stop between them leaves a record for a block that is already off, which the next run
//! clears. Changes that are not due are never touched. The clock is never read here.

use super::delay_arguments::{pick_zone, read_delay_arguments, Command};
use super::request_files::read_request_files;
use super::settle_report::{draft, halfway, nothing_waiting, saved, would_change};
use super::waiting_file_disk::save_waiting;
use crate::add_block::file_replace::{save_file, RealSteps, Steps, SETTINGS_WORDS};
use crate::session::run_command::{fail, write_result, EXIT_NOT_UNDERSTOOD, EXIT_REFUSED};
use crate::session::session_arguments::USAGE;
use crate::session::session_report::{quoted, refusal_line};
use leko_core::delayed_changes::settle_plan::settle;
use std::io::Write;

/// Runs the settle request in `words` (the words after `settle`) at the moment `now_ms`, in the zone
/// `--zone` names or else `computer_zone`, and returns the exit code.
pub fn run_settle(words: &[String], now_ms: i64, computer_zone: Option<&str>, out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    run_settle_with(words, now_ms, computer_zone, &mut RealSteps, out, err)
}

/// Like [`run_settle`], saving through `steps` (a test makes a step of a save fail).
pub fn run_settle_with(words: &[String], now_ms: i64, computer_zone: Option<&str>, steps: &mut dyn Steps, out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    let request = match read_delay_arguments(words, Command::Settle) {
        Ok(request) => request,
        Err(problem) => return fail(err, EXIT_NOT_UNDERSTOOD, &format!("{problem}\n{USAGE}\n")),
    };
    let (zone_name, zone) = match pick_zone(request.zone.as_deref(), computer_zone) {
        Ok(chosen) => chosen,
        Err((code, text)) => return fail(err, code, &text),
    };
    let files = match read_request_files(&request.file, err) {
        Ok(files) => files,
        Err(code) => return code,
    };
    if files.changes.is_empty() {
        return write_result(out, err, &nothing_waiting());
    }
    let settled = match settle(&files.bytes, &files.changes, now_ms, &zone) {
        Ok(settled) => settled,
        Err(sentence) => return fail(err, EXIT_REFUSED, &refusal_line(&sentence)),
    };
    let command = format!("leko settle {}", quoted(&request.file));
    if !request.write || !would_change(&settled.steps) {
        return write_result(out, err, &draft(&settled.steps, &zone_name, &zone, &command));
    }
    if let Some(new_bytes) = &settled.bytes {
        if let Err(sentence) = save_file(&request.file, new_bytes, steps, SETTINGS_WORDS, false) {
            return fail(err, EXIT_REFUSED, &format!("{sentence}\n"));
        }
    }
    match save_waiting(&files.waiting_file, &settled.remaining, steps) {
        Ok(()) => write_result(out, err, &saved(&settled.steps, &zone_name, &zone, &request.file, &files.waiting_file)),
        Err(sentence) if settled.bytes.is_some() && settled.steps.iter().any(|step| matches!(step.change.kind, leko_core::delayed_changes::waiting_change::ChangeKind::EditCoverage | leko_core::delayed_changes::waiting_change::ChangeKind::EditSchedule)) => {
            fail(err, EXIT_REFUSED, &format!("{sentence} The settings file was already saved. Run {command} --write again to clear applied changes.\n"))
        }
        Err(sentence) if settled.bytes.is_some() => fail(err, EXIT_REFUSED, &halfway(&sentence, &request.file, &command)),
        Err(sentence) => fail(err, EXIT_REFUSED, &format!("{sentence} Nothing was saved.\n")),
    }
}
