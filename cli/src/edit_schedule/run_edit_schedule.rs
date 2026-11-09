//! Preview or save one schedule edit using file and waiting-queue safeguards.
use super::schedule_arguments::{parse, USAGE};
use crate::add_block::file_replace::{save_file, RealSteps, Steps, SETTINGS_WORDS};
use crate::delayed_changes::waiting_file_disk::save_waiting;
use crate::edit_prologue::phone_compatibility_warning;
use crate::session::run_command::{fail, write_result, EXIT_REFUSED};
use crate::session::session_report::{quoted, refusal_line};
use leko_core::delayed_changes::switch_off_plan::{request_moment, SwitchOffPlan};
use leko_core::delayed_changes::waiting_change::WaitingChange;
use leko_core::edit_schedule::rule_edit::{apply_schedule_zip, ScheduleApply};
use leko_core::edit_schedule::schedule_plan::plan_schedule_edit;
use serde_json::Value;
use std::io::Write;

pub fn run_edit_schedule(
    words: &[String],
    now_ms: i64,
    computer_zone: Option<&str>,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    run_edit_schedule_with(words, now_ms, computer_zone, &mut RealSteps, out, err)
}

pub fn run_edit_schedule_with(
    words: &[String],
    now_ms: i64,
    computer_zone: Option<&str>,
    steps: &mut dyn Steps,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    let prologue = load_edit_prologue_or_return!(words, USAGE, parse, computer_zone, err);
    let (request, zone_name, zone, bytes, set, waiting_file) = (prologue.request, prologue.zone_name, prologue.zone, prologue.bytes, prologue.set, prologue.waiting_file);
    let mut waiting = prologue.waiting;
    if waiting.iter().any(|change| change.block == request.id) {
        return fail(
            err,
            EXIT_REFUSED,
            &refusal_line("A change is already waiting for this block; use leko cancel or leko settle first"),
        );
    }
    let (edited, old_doc, new_doc) = match apply_schedule_zip(
        &bytes,
        &request.id,
        &request.rule,
        request.days.as_deref(),
        request.stretches.as_deref(),
    ) {
        Ok(ScheduleApply::AlreadyApplied) => {
            let warning = phone_compatibility_warning(&bytes).map(|w| format!("\n{w}")).unwrap_or_default();
            return write_result(out, err, &format!("Schedule is already as requested. Nothing was saved.{warning}\n"));
        }
        Ok(ScheduleApply::Changed { bytes, old, new }) => (bytes, old, new),
        Err(problem) => return fail(err, EXIT_REFUSED, &refusal_line(&problem)),
    };

    let plan = match plan_schedule_edit(&set, &request.id, &request.rule, &old_doc, &new_doc, now_ms, &zone) {
        Ok(plan) => plan,
        Err(problem) => return fail(err, EXIT_REFUSED, &refusal_line(&problem)),
    };

    let outcome = match &plan {
        SwitchOffPlan::LandsNow => "The change can land now.".to_string(),
        SwitchOffPlan::Waits { due_at, because } => format!(
            "The change must wait until {} ({zone_name}) because {}.",
            zone.local_text(*due_at),
            because.join(", ")
        ),
        SwitchOffPlan::NeedsPartnerKey => return fail(err, EXIT_REFUSED, &refusal_line("The change needs a partner key")),
        SwitchOffPlan::CannotLand => return fail(err, EXIT_REFUSED, &refusal_line("The locks cannot be satisfied automatically")),
        _ => return fail(err, EXIT_REFUSED, &refusal_line("The block could not be judged")),
    };

    let old_rules = old_doc.get("rules").and_then(Value::as_array).unwrap();
    let new_rules = new_doc.get("rules").and_then(Value::as_array).unwrap();
    let old_rule = old_rules.iter().find(|r| r.get("id").and_then(Value::as_str) == Some(&request.rule)).unwrap();
    let new_rule = new_rules.iter().find(|r| r.get("id").and_then(Value::as_str) == Some(&request.rule)).unwrap();

    let warning = phone_compatibility_warning(&edited).map(|w| format!("\n{w}")).unwrap_or_default();
    let summary = format!(
        "Block {} rule {}\nCurrent rule: {}\nProposed rule: {}\n{outcome}{warning}",
        quoted(&request.id),
        quoted(&request.rule),
        old_rule,
        new_rule
    );

    if !request.write {
        let command = std::iter::once("leko edit-schedule".to_string())
            .chain(words.iter().map(|word| quoted(word)))
            .collect::<Vec<_>>()
            .join(" ");
        return write_result(out, err, &format!("{summary}\nNothing was saved. To save: {command} --write\n"));
    }

    let saved = match plan {
        SwitchOffPlan::LandsNow => {
            save_file(&request.file, &edited, steps, SETTINGS_WORDS, false).map(|_| "Saved.".to_string())
        }
        SwitchOffPlan::Waits { due_at, .. } => {
            match WaitingChange::schedule(
                &request.id,
                &request.rule,
                old_rule.clone(),
                new_rule.clone(),
                request_moment(now_ms),
                due_at,
            ) {
                Ok(change) => {
                    waiting.push(change);
                    save_waiting(&waiting_file, &waiting, steps).map(|_| "Waiting; use leko settle when due.".to_string())
                }
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
