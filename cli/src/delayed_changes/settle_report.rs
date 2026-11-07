//! The sentences of `leko settle`: nothing waiting, one line for what happens to each parked change
//! (due, not yet due and why, cleared, stopped), the draft, the saved result and the halfway
//! failure. Moments are shown on the wall clock of the chosen zone. These functions only build text;
//! each ends with a newline.

use crate::session::session_report::shown;
use leko_core::config_time::zone_time::ZoneTime;
use leko_core::delayed_changes::settle_plan::{SettleOutcome, SettleStep, StopReason};
use leko_core::delayed_changes::waiting_change::ChangeKind;

pub fn nothing_waiting() -> String {
    "Nothing is waiting. Nothing was saved.\n".to_string()
}

/// One line for `step`; `done` says whether it was made (saved) or only drafted.
pub fn step_line(step: &SettleStep, zone_name: &str, zone: &ZoneTime, done: bool) -> String {
    let block = shown(&step.change.block);
    if step.change.kind == ChangeKind::EditSchedule {
        let rule = step.change.rule.as_deref().unwrap_or("rule");
        let sentence = match &step.outcome {
            SettleOutcome::Lands if done => format!("The schedule edit for block {block} rule {rule} was saved."),
            SettleOutcome::Lands => format!("The schedule edit for block {block} rule {rule} is due."),
            SettleOutcome::NotYet { due_at, because } => format!("The schedule edit for block {block} rule {rule} may land from {} ({zone_name}), held by: {}.", zone.local_text(*due_at), because.join("; ")),
            SettleOutcome::AlreadyApplied => format!("The schedule edit for block {block} is already applied; its waiting record is cleared."),
            SettleOutcome::Stopped(StopReason::BlockMissing) => format!("Block {block} is missing; its schedule edit is kept."),
            SettleOutcome::Stopped(StopReason::EditConflict) => format!("The schedule edit for block {block} conflicts with current settings; it is kept."),
            SettleOutcome::Stopped(StopReason::NeedsPartnerKey) => format!("The schedule edit for block {block} needs a partner key; it is kept."),
            SettleOutcome::Stopped(StopReason::CannotLand) => format!("The schedule edit for block {block} cannot land automatically; it is kept."),
            _ => format!("The schedule edit for block {block} could not be settled; it is kept."),
        };
        let due_at = match &step.outcome { SettleOutcome::NotYet { due_at, .. } => *due_at, _ => step.change.due_at };
        return format!("{sentence} Due time: {} ({zone_name}).", zone.local_text(due_at));
    }
    if matches!(step.change.kind, ChangeKind::EditCoverage | ChangeKind::EditSchedule) {
        let operations = step.change.operations.iter().map(|op| format!("{} {}", op.action.word(), shown(&op.target))).collect::<Vec<_>>().join(", ");
        let sentence = match &step.outcome {
            SettleOutcome::Lands if done => format!("The coverage edit for block {block} was saved: {operations}."),
            SettleOutcome::Lands => format!("The coverage edit for block {block} is due: {operations}."),
            SettleOutcome::NotYet { due_at, because } => format!("The coverage edit for block {block} ({operations}) may land from {} ({zone_name}), held by: {}.", zone.local_text(*due_at), because.join("; ")),
            SettleOutcome::AlreadyApplied => format!("The coverage edit for block {block} is already applied; its waiting record is cleared."),
            SettleOutcome::Stopped(StopReason::BlockMissing) => format!("Block {block} is missing; its coverage edit is kept."),
            SettleOutcome::Stopped(StopReason::EditConflict) => format!("The coverage edit for block {block} conflicts with current settings; it is kept."),
            SettleOutcome::Stopped(StopReason::NeedsPartnerKey) => format!("The coverage edit for block {block} needs a partner key; it is kept."),
            SettleOutcome::Stopped(StopReason::CannotLand) => format!("The coverage edit for block {block} cannot land automatically; it is kept."),
            _ => format!("The coverage edit for block {block} could not be settled; it is kept."),
        };
        let due_at = match &step.outcome { SettleOutcome::NotYet { due_at, .. } => *due_at, _ => step.change.due_at };
        return format!("{sentence} Due time: {} ({zone_name}).", zone.local_text(due_at));
    }
    match &step.outcome {
        SettleOutcome::Lands if done => format!("The block {block} is switched off."),
        SettleOutcome::Lands => format!("The block {block} is due and would be switched off."),
        SettleOutcome::NotYet { due_at, because } => {
            format!("The block {block} is not due yet: it may be switched off from {} ({zone_name}), held by: {}.", zone.local_text(*due_at), because.join("; "))
        }
        SettleOutcome::BlockGone => format!("The block {block} is no longer in the settings, so its waiting change is cleared."),
        SettleOutcome::AlreadyOff => format!("The block {block} is already off, so its waiting change is cleared."),
        SettleOutcome::Stopped(StopReason::NeedsPartnerKey) => {
            format!("The block {block} now has a lock that needs a partner's key, so it was not switched off; its waiting change is kept.")
        }
        SettleOutcome::Stopped(StopReason::CannotLand) => format!("The locks on the block {block} never leave a moment to change it, so its waiting change is kept."),
        SettleOutcome::AlreadyApplied | SettleOutcome::Stopped(StopReason::EditConflict | StopReason::BlockMissing) => format!("The waiting change for block {block} could not be settled."),
    }
}

/// True when saving would change a file: a block would land or a record would be cleared.
pub fn would_change(steps: &[SettleStep]) -> bool {
    steps.iter().any(|step| matches!(step.outcome, SettleOutcome::Lands | SettleOutcome::BlockGone | SettleOutcome::AlreadyOff | SettleOutcome::AlreadyApplied))
}

fn lines(steps: &[SettleStep], zone_name: &str, zone: &ZoneTime, done: bool) -> String {
    steps.iter().map(|step| step_line(step, zone_name, zone, done) + "\n").collect()
}

/// What settle found, without saving anything: every line, then what to run to save it.
pub fn draft(steps: &[SettleStep], zone_name: &str, zone: &ZoneTime, command: &str) -> String {
    let found = lines(steps, zone_name, zone, false);
    if !would_change(steps) {
        return format!("{found}Nothing is due yet. Nothing was saved.\n");
    }
    format!("{found}Draft: nothing was saved. To save this, run: {command} --write\n")
}

/// What settle did, after both files were saved.
pub fn saved(steps: &[SettleStep], zone_name: &str, zone: &ZoneTime, settings_file: &str, waiting_file: &str) -> String {
    let count = |wanted: fn(&SettleOutcome) -> bool| steps.iter().filter(|step| wanted(&step.outcome)).count();
    let landed = count(|outcome| matches!(outcome, SettleOutcome::Lands));
    let cleared = count(|outcome| matches!(outcome, SettleOutcome::BlockGone | SettleOutcome::AlreadyOff | SettleOutcome::AlreadyApplied));
    let kept = steps.len() - landed - cleared;
    if steps.iter().any(|step| matches!(step.change.kind, ChangeKind::EditCoverage | ChangeKind::EditSchedule)) {
        return format!("{}Saved: {landed} changes in {}, {cleared} cleared and {kept} still waiting in {}.\n", lines(steps, zone_name, zone, true), shown(settings_file), shown(waiting_file));
    }
    format!(
        "{}Saved: {landed} switched off in {}, {cleared} cleared and {kept} still waiting in {}.\n",
        lines(steps, zone_name, zone, true),
        shown(settings_file),
        shown(waiting_file)
    )
}

/// The settings file was saved but the waiting file was not: the blocks are off, the records stay.
pub fn halfway(waiting_sentence: &str, settings_file: &str, command: &str) -> String {
    format!("{waiting_sentence} The settings file {} was already saved, so the due blocks are off. Run {command} --write again to clear their waiting changes.\n", shown(settings_file))
}
