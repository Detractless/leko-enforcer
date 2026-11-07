//! Preview or save one coverage edit using the existing file and waiting-queue safeguards.
use super::edit_arguments::parse;
use crate::add_block::file_replace::{save_file, RealSteps, Steps, SETTINGS_WORDS};
use crate::delayed_changes::delay_arguments::pick_zone;
use crate::delayed_changes::waiting_file_disk::{read_waiting_changes, save_waiting, waiting_path};
use crate::session::run_command::{fail, write_result, EXIT_NOT_UNDERSTOOD, EXIT_REFUSED};
use crate::session::session_report::{quoted, refusal_line};
use crate::status::settings_file::read_settings_and_bytes;
use leko_core::delayed_changes::switch_off_plan::{request_moment, SwitchOffPlan};
use leko_core::delayed_changes::waiting_change::WaitingChange;
use leko_core::edit_block::coverage::{apply_coverage_zip, CoverageApply};
use leko_core::edit_block::edit_plan::plan_edit;
use leko_core::io::zip_check::{check_zip_in_mode, ZipVerdict};
use leko_core::models::specs::ReadMode;
use std::collections::HashSet;
use std::io::Write;

const USAGE: &str = "Usage: leko edit-block FILE --id ID [--zone ZONE] (--add-app ID | --remove-app ID | --add-site HOST | --remove-site HOST)... [--write]";

fn phone_warning(bytes: &[u8]) -> Option<String> {
    match check_zip_in_mode(bytes, &HashSet::new(), ReadMode::Phone) {
        ZipVerdict::Read(_) => None,
        ZipVerdict::Refused(reason) => Some(format!("Phone Enforcer 0.3 cannot read these settings: {}.", reason.sentence)),
    }
}

pub fn run_edit_block(words: &[String], now_ms: i64, computer_zone: Option<&str>, out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    run_edit_block_with(words, now_ms, computer_zone, &mut RealSteps, out, err)
}

pub fn run_edit_block_with(words: &[String], now_ms: i64, computer_zone: Option<&str>, steps: &mut dyn Steps, out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    let request = match parse(words) {
        Ok(request) => request,
        Err(problem) => return fail(err, EXIT_NOT_UNDERSTOOD, &format!("{problem}\n{USAGE}\n")),
    };
    let (zone_name, zone) = match pick_zone(request.zone.as_deref(), computer_zone) {
        Ok(zone) => zone,
        Err((code, text)) => return fail(err, code, &text),
    };
    let (bytes, set) = match read_settings_and_bytes(&request.file) {
        Ok(value) => value,
        Err(problem) => return fail(err, EXIT_REFUSED, &refusal_line(&problem)),
    };
    let waiting_file = waiting_path(&request.file);
    let mut waiting = match read_waiting_changes(&waiting_file) {
        Ok(value) => value.unwrap_or_default(),
        Err(problem) => return fail(err, EXIT_REFUSED, &refusal_line(&problem)),
    };
    if waiting.iter().any(|change| change.block == request.id) {
        return fail(err, EXIT_REFUSED, &refusal_line("A change is already waiting for this block; use leko cancel or leko settle first"));
    }
    let (edited, old, new) = match apply_coverage_zip(&bytes, &request.id, &request.operations) {
        Ok(CoverageApply::AlreadyApplied) => {
            let warning = phone_warning(&bytes).map(|w| format!("\n{w}")).unwrap_or_default();
            return write_result(out, err, &format!("Coverage is already as requested. Nothing was saved.{warning}\n"));
        }
        Ok(CoverageApply::Conflict) => return fail(err, EXIT_REFUSED, &refusal_line("Some operations are already applied while others are not; review the request")),
        Ok(CoverageApply::Changed { bytes, old, new }) => (bytes, old, new),
        Err(problem) => return fail(err, EXIT_REFUSED, &refusal_line(&problem)),
    };
    let plan = match plan_edit(&set, &request.id, &old, &new, now_ms, &zone) {
        Ok(plan) => plan,
        Err(problem) => return fail(err, EXIT_REFUSED, &refusal_line(&problem)),
    };
    let outcome = match &plan {
        SwitchOffPlan::LandsNow => "The change can land now.".to_string(),
        SwitchOffPlan::Waits { due_at, because } => format!("The change must wait until {} ({zone_name}) because {}.", zone.local_text(*due_at), because.join(", ")),
        SwitchOffPlan::NeedsPartnerKey => return fail(err, EXIT_REFUSED, &refusal_line("The change needs a partner key")),
        SwitchOffPlan::CannotLand => return fail(err, EXIT_REFUSED, &refusal_line("The locks cannot be satisfied automatically")),
        _ => return fail(err, EXIT_REFUSED, &refusal_line("The block could not be judged")),
    };
    let warning = phone_warning(&edited).map(|w| format!("\n{w}")).unwrap_or_default();
    let summary = format!("Block {}\nCurrent coverage: {}\nProposed coverage: {}\n{outcome}{warning}", quoted(&request.id), old["covers"], new["covers"]);
    if !request.write {
        let command = std::iter::once("leko edit-block".to_string()).chain(words.iter().map(|word| quoted(word))).collect::<Vec<_>>().join(" ");
        return write_result(out, err, &format!("{summary}\nNothing was saved. To save: {command} --write\n"));
    }
    let saved = match plan {
        SwitchOffPlan::LandsNow => save_file(&request.file, &edited, steps, SETTINGS_WORDS, false).map(|_| "Saved.".to_string()),
        SwitchOffPlan::Waits { due_at, .. } => {
            match WaitingChange::edit(&request.id, request_moment(now_ms), due_at, request.operations) {
                Ok(change) => { waiting.push(change); save_waiting(&waiting_file, &waiting, steps).map(|_| "Waiting; use leko settle when due.".to_string()) }
                Err(problem) => Err(problem),
            }
        }
        _ => unreachable!(),
    };
    match saved {
        Ok(message) => write_result(out, err, &format!("{summary}\n{message}\n")),
        Err(problem) => fail(err, EXIT_REFUSED, &refusal_line(&problem)),
    }
}
