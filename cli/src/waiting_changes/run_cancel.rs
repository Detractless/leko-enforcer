//! Runs one `leko cancel` request in order: the words (exit 2), then the waiting file beside the
//! settings file (exit 1 when it is bad; no file, or no record for the block, means nothing is
//! waiting and no file is made), then the core takes the block's record out. The result is a draft;
//! with `--write` the shortened list is saved in one step, so a failure leaves the old file as it was.
//! Taking a request back is a tightening: no wait, no key, and the settings zip is never opened, so
//! a damaged or missing zip cannot stop it. The clock is never read here.

use super::cancel_arguments::read_cancel_arguments;
use super::cancel_report::{cancelled, draft, nothing_to_cancel};
use crate::add_block::file_replace::{RealSteps, Steps};
use crate::delayed_changes::waiting_file_disk::{read_waiting_changes, save_waiting, waiting_path};
use crate::session::run_command::{fail, write_result, EXIT_NOT_UNDERSTOOD, EXIT_REFUSED};
use crate::session::session_arguments::USAGE;
use crate::session::session_report::{quoted, refusal_line};
use leko_core::waiting_changes::cancel_waiting::cancel_waiting;
use std::io::Write;

/// Runs the cancel request in `words` (the words after `cancel`) and returns the exit code.
pub fn run_cancel(words: &[String], out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    run_cancel_with(words, &mut RealSteps, out, err)
}

/// Like [`run_cancel`], saving through `steps` (a test makes a step of a save fail).
pub fn run_cancel_with(words: &[String], steps: &mut dyn Steps, out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    let request = match read_cancel_arguments(words) {
        Ok(request) => request,
        Err(problem) => return fail(err, EXIT_NOT_UNDERSTOOD, &format!("{problem}\n{USAGE}\n")),
    };
    let waiting_file = waiting_path(&request.file);
    let changes = match read_waiting_changes(&waiting_file) {
        Ok(found) => found.unwrap_or_default(),
        Err(sentence) => return fail(err, EXIT_REFUSED, &refusal_line(&sentence)),
    };
    let done = cancel_waiting(&changes, &request.id);
    if done.removed.is_none() {
        return write_result(out, err, &nothing_to_cancel(&request.id));
    }
    if !request.write {
        let command = format!("leko cancel {} --id {}", quoted(&request.file), quoted(&request.id));
        return write_result(out, err, &draft(&request.id, &command));
    }
    match save_waiting(&waiting_file, &done.remaining, steps) {
        Ok(()) => write_result(out, err, &cancelled(&request.id, &waiting_file)),
        Err(sentence) => fail(err, EXIT_REFUSED, &format!("{sentence}\n")),
    }
}
