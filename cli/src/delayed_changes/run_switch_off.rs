//! Runs one `leko switch-off` request in order: the words (exit 2 when they are not understood,
//! before any file is touched), the zone, the settings file and the waiting file (exit 1 when either
//! is bad, and a bad waiting file is never replaced), then the core's plan. The plan is shown as a
//! draft; with `--write` a change that lands now is saved into the zip and a change that must wait is
//! parked in the waiting file, one file and one step at a time. Asking twice changes nothing. The
//! moment and the computer's zone come in as arguments; the clock is never read here.

use super::delay_arguments::{pick_zone, read_delay_arguments, Command};
use super::switch_off_report::{already_off, already_waiting, cannot_land, draft_now, draft_waits, needs_partner_key, no_such_block, parked, saved_now, Wait};
use super::waiting_file_disk::{read_waiting_changes, save_waiting, waiting_path};
use crate::add_block::file_replace::{save_file, RealSteps, Steps, SETTINGS_WORDS};
use crate::session::run_command::{fail, write_result, EXIT_NOT_UNDERSTOOD, EXIT_REFUSED};
use crate::session::session_arguments::USAGE;
use crate::session::session_report::{quoted, refusal_line};
use crate::status::settings_file::read_settings_and_bytes;
use leko_core::delayed_changes::block_off_zip::switch_block_off;
use leko_core::delayed_changes::switch_off_plan::{plan_switch_off, request_moment, SwitchOffPlan};
use leko_core::delayed_changes::waiting_change::{ChangeKind, WaitingChange};
use std::io::Write;

/// Runs the switch-off request in `words` (the words after `switch-off`) at the moment `now_ms`, in
/// the zone `--zone` names or else `computer_zone`, and returns the exit code.
pub fn run_switch_off(words: &[String], now_ms: i64, computer_zone: Option<&str>, out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    run_switch_off_with(words, now_ms, computer_zone, &mut RealSteps, out, err)
}

/// Like [`run_switch_off`], saving through `steps` (a test makes a step of the save fail).
pub fn run_switch_off_with(words: &[String], now_ms: i64, computer_zone: Option<&str>, steps: &mut dyn Steps, out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    let request = match read_delay_arguments(words, Command::SwitchOff) {
        Ok(request) => request,
        Err(problem) => return fail(err, EXIT_NOT_UNDERSTOOD, &format!("{problem}\n{USAGE}\n")),
    };
    let (zone_name, zone) = match pick_zone(request.zone.as_deref(), computer_zone) {
        Ok(chosen) => chosen,
        Err((code, text)) => return fail(err, code, &text),
    };
    let id = request.id.as_deref().unwrap_or_default();
    let (bytes, set) = match read_settings_and_bytes(&request.file) {
        Ok(read) => read,
        Err(sentence) => return fail(err, EXIT_REFUSED, &refusal_line(&sentence)),
    };
    let waiting_file = waiting_path(&request.file);
    let changes = match read_waiting_changes(&waiting_file) {
        Ok(found) => found.unwrap_or_default(),
        Err(sentence) => return fail(err, EXIT_REFUSED, &refusal_line(&sentence)),
    };
    if changes.iter().any(|change| change.block == id && change.kind != ChangeKind::SwitchOff) {
        return fail(err, EXIT_REFUSED, &refusal_line("Another change is waiting for this block; use leko cancel or leko settle first"));
    }
    let parked_already = changes.iter().find(|change| change.kind == ChangeKind::SwitchOff && change.block == id);
    let command = std::iter::once("leko switch-off".to_string()).chain(words.iter().map(|word| quoted(word))).collect::<Vec<_>>().join(" ");
    let local = |ms: i64| zone.local_text(ms);
    match (plan_switch_off(&set, id, now_ms, &zone), parked_already) {
        (SwitchOffPlan::NoSuchBlock, _) => fail(err, EXIT_REFUSED, &refusal_line(&no_such_block(id, &request.file))),
        (SwitchOffPlan::AlreadyOff, _) => write_result(out, err, &already_off(id)),
        (SwitchOffPlan::NeedsPartnerKey, _) => fail(err, EXIT_REFUSED, &refusal_line(&needs_partner_key(id))),
        (SwitchOffPlan::CannotLand, _) => fail(err, EXIT_REFUSED, &refusal_line(&cannot_land(id))),
        (_, Some(change)) => {
            let settle = format!("leko settle {} --write", quoted(&request.file));
            write_result(out, err, &already_waiting(id, &local(change.due_at), &zone_name, now_ms >= change.due_at, &settle))
        }
        (SwitchOffPlan::LandsNow, None) if !request.write => write_result(out, err, &draft_now(id, &command)),
        (SwitchOffPlan::LandsNow, None) => {
            let saved = switch_now(id, &request.file, &bytes, steps);
            finish(saved, saved_now(id, &request.file), out, err)
        }
        (SwitchOffPlan::Waits { due_at, because }, None) => {
            let wait = Wait { local: &local(due_at), zone: &zone_name, because: &because };
            if !request.write {
                return write_result(out, err, &draft_waits(id, &wait, &waiting_file, &command));
            }
            let settle = format!("leko settle {} --write", quoted(&request.file));
            let saved = park(id, due_at, now_ms, changes, &waiting_file, steps);
            finish(saved, parked(id, &wait, &waiting_file, &settle), out, err)
        }
    }
}

/// Tells `told` when the save worked; else the sentence saying which step failed, as an error.
fn finish(saved: Result<(), String>, told: String, out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    match saved {
        Ok(()) => write_result(out, err, &told),
        Err(text) => fail(err, EXIT_REFUSED, &text),
    }
}

/// Switches the block `id` off in the zip `bytes` and saves the zip over `file`; the error is the
/// text for stderr.
fn switch_now(id: &str, file: &str, bytes: &[u8], steps: &mut dyn Steps) -> Result<(), String> {
    let new_bytes = switch_block_off(bytes, id).map_err(|sentence| refusal_line(&sentence))?;
    save_file(file, &new_bytes, steps, SETTINGS_WORDS, false).map_err(|sentence| format!("{sentence}\n"))
}

/// Parks the change for the block `id`, which may land from `due_at`, in the waiting file; the other
/// changes already there keep their records. The error is the text for stderr.
fn park(id: &str, due_at: i64, now_ms: i64, mut changes: Vec<WaitingChange>, waiting_file: &str, steps: &mut dyn Steps) -> Result<(), String> {
    changes.push(WaitingChange::new(ChangeKind::SwitchOff, id, request_moment(now_ms), due_at).map_err(|sentence| refusal_line(&sentence))?);
    save_waiting(waiting_file, &changes, steps).map_err(|sentence| format!("{sentence}\n"))
}
