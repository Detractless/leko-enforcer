//! The parked changes `leko status` adds to its answer: the ones whose block covers the app or site
//! asked about, found by the core's own settle plan and written with settle's own sentences, so the
//! two commands never word the same change two ways. The waiting file is only read; a file that is
//! not there means nothing is waiting. In JSON each change is an object with the block, the kind,
//! an outcome word and the UTC moment it may land (`null` unless the change is not due yet).

use crate::delayed_changes::settle_report::step_line;
use crate::delayed_changes::waiting_file_disk::{read_waiting_changes, waiting_path};
use leko_core::config_time::zone_time::ZoneTime;
use leko_core::decision::block_cover::Target;
use leko_core::delayed_changes::settle_plan::{SettleOutcome, SettleStep, StopReason};
use leko_core::delayed_changes::waiting_change::ChangeKind;
use leko_core::models::specs::ReadSet;
use leko_core::sessions::utc_moment::format_utc_moment;
use leko_core::waiting_changes::waiting_for_target::waiting_for_target;
use serde_json::{json, Value};

/// The steps of the parked changes that belong to `target`, or the sentence for stderr saying the
/// waiting file beside `settings_file` could not be read (then no change is shown).
pub fn read_waiting_for(settings_file: &str, set: &ReadSet, target: Target, now_ms: i64, zone: &ZoneTime) -> Result<Vec<SettleStep>, String> {
    let changes = match read_waiting_changes(&waiting_path(settings_file)) {
        Ok(found) => found.unwrap_or_default(),
        Err(sentence) => return Err(format!("{sentence} The parked changes could not be listed.")),
    };
    Ok(waiting_for_target(set, target, &changes, now_ms, zone))
}

/// One line after another, each starting on a new line, for the sentence of a person.
pub fn waiting_lines(steps: &[SettleStep], zone_name: &str, zone: &ZoneTime) -> String {
    steps.iter().map(|step| format!("\n{}", step_line(step, zone_name, zone, false))).collect()
}

fn outcome_word(outcome: &SettleOutcome) -> &'static str {
    match outcome {
        SettleOutcome::Lands => "due",
        SettleOutcome::NotYet { .. } => "not-yet",
        SettleOutcome::BlockGone => "block-gone",
        SettleOutcome::AlreadyOff => "already-off",
        SettleOutcome::AlreadyApplied => "already-applied",
        SettleOutcome::Stopped(StopReason::NeedsPartnerKey) => "needs-partner-key",
        SettleOutcome::Stopped(StopReason::CannotLand) => "cannot-land",
        SettleOutcome::Stopped(StopReason::EditConflict) => "edit-conflict",
        SettleOutcome::Stopped(StopReason::BlockMissing) => "block-missing",
    }
}

/// The JSON for a script: a list with one object per change, or `null` when the file was unreadable.
pub fn waiting_json(steps: Option<&[SettleStep]>) -> Value {
    let Some(steps) = steps else { return Value::Null };
    let one = |step: &SettleStep| {
        let from = match &step.outcome {
            SettleOutcome::NotYet { due_at, .. } => format_utc_moment(*due_at),
            _ => None,
        };
        let mut value = json!({"block": step.change.block, "kind": step.change.kind.word(), "outcome": outcome_word(&step.outcome), "from": from});
        if step.change.kind == ChangeKind::EditSchedule {
            value["rule"] = json!(step.change.rule);
            if value["from"].is_null() { value["from"] = json!(format_utc_moment(step.change.due_at)); }
        }
        if step.change.kind == ChangeKind::EditCoverage {
            value["operations"] = json!(step.change.operations.iter().map(|op| json!({"action": op.action.word(), "target": op.target})).collect::<Vec<_>>());
            if value["from"].is_null() { value["from"] = json!(format_utc_moment(step.change.due_at)); }
        }
        value
    };
    Value::Array(steps.iter().map(one).collect())
}
