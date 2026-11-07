//! What happens to each parked change at one moment. The block is judged as it is now, by the same
//! plan a new request gets, so a lock tightened since the request counts; a change never lands
//! earlier than the later of the due moment saved with it and the moment the present locks give for
//! the original request moment, and never while the clock reads earlier than the request. A change
//! whose block is gone or already off is cleared; one that needs a partner's key, or that cannot land
//! by itself, is stopped and kept. Returns the settings with the due changes applied and the changes
//! still waiting. Nothing here reads the clock or a file: the moment and the zone are arguments.
use crate::edit_schedule::schedule_plan::plan_schedule_edit;
use serde_json::Value;

use super::block_off_zip::switch_block_off;
use super::switch_off_plan::{plan_switch_off, SwitchOffPlan};
use super::waiting_change::{ChangeKind, WaitingChange};
use crate::config_time::zone_time::ZoneTime;
use crate::edit_block::coverage::{apply_coverage_document, apply_coverage_zip, CoverageApply, CoverageDocument};
use crate::edit_block::edit_plan::plan_edit;
use crate::io::zip_check::{check_zip, ZipVerdict};
use crate::models::specs::ReadSet;
use std::collections::HashSet;

/// Why a change was stopped and kept.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopReason {
    NeedsPartnerKey,
    CannotLand,
    EditConflict,
    BlockMissing,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SettleOutcome {
    /// Due: the block is switched off in the new zip and the record is removed.
    Lands,
    /// Not due yet; it may land at `due_at`, held by `because`. The record is kept as it is.
    NotYet { due_at: i64, because: Vec<String> },
    /// The block is no longer in the settings; the record is removed.
    BlockGone,
    /// The block is already off; the record is removed.
    AlreadyOff,
    /// A previously saved edit already has every requested effect; clear its record.
    AlreadyApplied,
    /// The change cannot land now; the record is kept.
    Stopped(StopReason),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettleStep {
    pub change: WaitingChange,
    pub outcome: SettleOutcome,
}

/// The result of settling: what happened to each change in the order given, the zip with every due
/// change applied (`None` when nothing landed), and the changes that stay in the waiting file.
#[derive(Debug, Clone)]
pub struct Settled {
    pub steps: Vec<SettleStep>,
    pub bytes: Option<Vec<u8>>,
    pub remaining: Vec<WaitingChange>,
}

const SAVED_MOMENT: &str = "the moment saved when the change was asked for";

fn outcome_of(change: &WaitingChange, plan: SwitchOffPlan, now_ms: i64) -> SettleOutcome {
    let (given, because) = match plan {
        SwitchOffPlan::NoSuchBlock => return SettleOutcome::BlockGone,
        SwitchOffPlan::AlreadyOff => return SettleOutcome::AlreadyOff,
        SwitchOffPlan::NeedsPartnerKey => return SettleOutcome::Stopped(StopReason::NeedsPartnerKey),
        SwitchOffPlan::CannotLand => return SettleOutcome::Stopped(StopReason::CannotLand),
        SwitchOffPlan::LandsNow => (change.requested_at, Vec::new()),
        SwitchOffPlan::Waits { due_at, because } => (due_at, because),
    };
    let due_at = given.max(change.due_at);
    if now_ms >= due_at && now_ms >= change.requested_at {
        return SettleOutcome::Lands;
    }
    let because = if because.is_empty() || change.due_at > given { vec![SAVED_MOMENT.to_string()] } else { because };
    SettleOutcome::NotYet { due_at, because }
}

/// What happens to `change` at `now_ms` in `zone`, judging its block as the settings `set` hold it.
pub fn step_of(set: &ReadSet, change: &WaitingChange, now_ms: i64, zone: &ZoneTime) -> SettleStep {
    let outcome = match change.kind {
        ChangeKind::SwitchOff => outcome_of(change, plan_switch_off(set, &change.block, change.requested_at, zone), now_ms),
        ChangeKind::EditCoverage => edit_outcome(set, change, now_ms, zone),
        ChangeKind::EditSchedule => schedule_outcome(set, change, now_ms, zone),
    };
    SettleStep { change: change.clone(), outcome }
}

fn edit_outcome(set: &ReadSet, change: &WaitingChange, now_ms: i64, zone: &ZoneTime) -> SettleOutcome {
    if set.blocks.get(&change.block).is_none() { return SettleOutcome::Stopped(StopReason::BlockMissing); }
    let path = format!("blocks/{}.json", change.block);
    let Some((_, old)) = set.docs.iter().find(|(name, _)| *name == path) else { return SettleOutcome::Stopped(StopReason::EditConflict) };
    match apply_coverage_document(old, &change.operations) {
        Ok(CoverageDocument::AlreadyApplied) => SettleOutcome::AlreadyApplied,
        Ok(CoverageDocument::Conflict) | Err(_) => SettleOutcome::Stopped(StopReason::EditConflict),
        Ok(CoverageDocument::Changed(new)) => match plan_edit(set, &change.block, old, &new, change.requested_at, zone) {
            Ok(plan) => outcome_of(change, plan, now_ms),
            Err(_) => SettleOutcome::Stopped(StopReason::EditConflict),
        }
    }
}

fn schedule_outcome(set: &ReadSet, change: &WaitingChange, now_ms: i64, zone: &ZoneTime) -> SettleOutcome {
    if set.blocks.get(&change.block).is_none() { return SettleOutcome::Stopped(StopReason::BlockMissing); }
    let path = format!("blocks/{}.json", change.block);
    let Some((_, current_doc)) = set.docs.iter().find(|(name, _)| *name == path) else { return SettleOutcome::Stopped(StopReason::EditConflict) };
    let (Some(rule_id), Some(before_rule), Some(after_rule)) = (&change.rule, &change.before, &change.after) else {
        return SettleOutcome::Stopped(StopReason::EditConflict);
    };
    let rules = match current_doc.get("rules").and_then(Value::as_array) {
        Some(r) => r,
        None => return SettleOutcome::Stopped(StopReason::EditConflict),
    };
    let current_rule = match rules.iter().find(|r| r.get("id").and_then(Value::as_str) == Some(rule_id)) {
        Some(r) => r,
        None => return SettleOutcome::Stopped(StopReason::EditConflict),
    };
    if current_rule == after_rule {
        return SettleOutcome::AlreadyApplied;
    }
    if current_rule != before_rule {
        return SettleOutcome::Stopped(StopReason::EditConflict);
    }
    let mut new_doc = current_doc.clone();
    let rules_mut = new_doc.get_mut("rules").and_then(Value::as_array_mut).unwrap();
    let idx = rules_mut.iter().position(|r| r.get("id").and_then(Value::as_str) == Some(rule_id)).unwrap();
    rules_mut[idx] = after_rule.clone();

    match plan_schedule_edit(set, &change.block, rule_id, current_doc, &new_doc, change.requested_at, zone) {
        Ok(plan) => outcome_of(change, plan, now_ms),
        Err(_) => SettleOutcome::Stopped(StopReason::EditConflict),
    }
}

/// Settles `changes` against the zip `bytes` at `now_ms` in `zone`, or gives the sentence saying why
/// the zip cannot be read or changed (then nothing is settled).
pub fn settle(bytes: &[u8], changes: &[WaitingChange], now_ms: i64, zone: &ZoneTime) -> Result<Settled, String> {
    let set = match check_zip(bytes, &HashSet::new()) {
        ZipVerdict::Read(read) => read.set,
        ZipVerdict::Refused(refusal) => return Err(format!("The settings file was refused: {}", refusal.sentence)),
    };
    let steps: Vec<SettleStep> = changes.iter().map(|change| step_of(&set, change, now_ms, zone)).collect();
    let mut current: Option<Vec<u8>> = None;
    for step in steps.iter().filter(|step| step.outcome == SettleOutcome::Lands) {
        let input = current.as_deref().unwrap_or(bytes);
        current = Some(match step.change.kind {
            ChangeKind::SwitchOff => switch_block_off(input, &step.change.block)?,
            ChangeKind::EditCoverage => match apply_coverage_zip(input, &step.change.block, &step.change.operations)? {
                CoverageApply::Changed { bytes, .. } => bytes,
                CoverageApply::AlreadyApplied | CoverageApply::Conflict => return Err("The waiting edit changed while settling; nothing was saved.".into()),
            },
            ChangeKind::EditSchedule => {
                let (Some(rule_id), Some(before_rule), Some(after_rule)) = (&step.change.rule, &step.change.before, &step.change.after) else {
                    return Err("The waiting schedule edit is missing rule data; nothing was saved.".into());
                };
                let mut files = crate::io::zip_reader::open_zip(input).map_err(|why| format!("The settings file was refused: {why}"))?.files;
                let path = format!("blocks/{}.json", step.change.block);
                let entry = files.iter_mut().find(|(name, _)| name == &path).ok_or("The block file is missing.")?;
                let mut doc: Value = serde_json::from_slice(&entry.1).map_err(|e| e.to_string())?;
                let rules = doc.get_mut("rules").and_then(Value::as_array_mut).ok_or("Invalid rules")?;
                let idx = rules.iter().position(|r| r.get("id").and_then(Value::as_str) == Some(rule_id)).ok_or("Rule missing")?;
                if &rules[idx] != before_rule && &rules[idx] != after_rule {
                    return Err("The waiting schedule edit precondition changed while settling; nothing was saved.".into());
                }
                rules[idx] = after_rule.clone();
                let mut text = serde_json::to_string_pretty(&doc).map_err(|e| e.to_string())?;
                text.push('\n');
                entry.1 = text.into_bytes();
                let result = crate::io::zip_writer::write_zip(&files).map_err(|why| format!("The edited zip could not be made: {why}"))?;
                match check_zip(&result, &HashSet::new()) {
                    ZipVerdict::Read(_) => result,
                    ZipVerdict::Refused(reason) => return Err(format!("The edited settings were refused: {}", reason.sentence)),
                }
            },
        });
    }
    let remaining = steps.iter().filter(|step| matches!(step.outcome, SettleOutcome::NotYet { .. } | SettleOutcome::Stopped(_))).map(|step| step.change.clone()).collect();
    Ok(Settled { steps, bytes: current, remaining })
}
