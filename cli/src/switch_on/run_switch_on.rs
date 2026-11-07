//! Runs one `leko switch-on` request in order: the words (exit 2 when they are not understood, before
//! any file is touched), the settings file and the waiting file (exit 1 when either is bad, and a
//! waiting file is never replaced, repaired or made), then the core's plan. Switching on is a
//! tightening, so it needs no wait, no key and no clock: the plan is shown as a draft, and with
//! `--write` the block's `on` field is saved into the zip, one file and one step at a time. A block
//! whose switch-off is still parked is refused, since the parked change would switch it off again.

use super::switch_on_arguments::read_switch_on_arguments;
use super::switch_on_report::{already_on, draft_on, saved_on, switch_off_waiting};
use crate::add_block::file_replace::{save_file, RealSteps, Steps, SETTINGS_WORDS};
use crate::delayed_changes::switch_off_report::no_such_block;
use crate::delayed_changes::waiting_file_disk::{read_waiting_changes, waiting_path};
use crate::session::run_command::{fail, write_result, EXIT_NOT_UNDERSTOOD, EXIT_REFUSED};
use crate::session::session_arguments::USAGE;
use crate::session::session_report::{quoted, refusal_line};
use crate::status::settings_file::read_settings_and_bytes;
use leko_core::delayed_changes::waiting_change::ChangeKind;
use leko_core::switch_on::block_on_zip::switch_block_on;
use leko_core::switch_on::plan_switch_on::{plan_switch_on, SwitchOnPlan};
use std::io::Write;

/// Runs the switch-on request in `words` (the words after `switch-on`) and returns the exit code.
pub fn run_switch_on(words: &[String], out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    run_switch_on_with(words, &mut RealSteps, out, err)
}

/// Like [`run_switch_on`], saving through `steps` (a test makes a step of the save fail).
pub fn run_switch_on_with(words: &[String], steps: &mut dyn Steps, out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    let request = match read_switch_on_arguments(words) {
        Ok(request) => request,
        Err(problem) => return fail(err, EXIT_NOT_UNDERSTOOD, &format!("{problem}\n{USAGE}\n")),
    };
    let (bytes, set) = match read_settings_and_bytes(&request.file) {
        Ok(read) => read,
        Err(sentence) => return fail(err, EXIT_REFUSED, &refusal_line(&sentence)),
    };
    let changes = match read_waiting_changes(&waiting_path(&request.file)) {
        Ok(found) => found.unwrap_or_default(),
        Err(sentence) => return fail(err, EXIT_REFUSED, &refusal_line(&sentence)),
    };
    let id = request.id.as_str();
    let parked = changes.iter().any(|change| change.kind == ChangeKind::SwitchOff && change.block == id);
    let command = std::iter::once("leko switch-on".to_string()).chain(words.iter().map(|word| quoted(word))).collect::<Vec<_>>().join(" ");
    match plan_switch_on(&set, id) {
        SwitchOnPlan::NoSuchBlock => fail(err, EXIT_REFUSED, &refusal_line(&no_such_block(id, &request.file))),
        _ if parked => fail(err, EXIT_REFUSED, &refusal_line(&switch_off_waiting(id, &request.file))),
        SwitchOnPlan::AlreadyOn => write_result(out, err, &already_on(id)),
        SwitchOnPlan::LandsNow if !request.write => write_result(out, err, &draft_on(id, &command)),
        SwitchOnPlan::LandsNow => match switch_now(id, &request.file, &bytes, steps) {
            Ok(()) => write_result(out, err, &saved_on(id, &request.file)),
            Err(text) => fail(err, EXIT_REFUSED, &text),
        },
    }
}

/// Switches the block `id` on in the zip `bytes` and saves the zip over `file`; the error is the text
/// for stderr.
fn switch_now(id: &str, file: &str, bytes: &[u8], steps: &mut dyn Steps) -> Result<(), String> {
    let new_bytes = switch_block_on(bytes, id).map_err(|sentence| refusal_line(&sentence))?;
    save_file(file, &new_bytes, steps, SETTINGS_WORDS, false).map_err(|sentence| format!("{sentence}\n"))
}
