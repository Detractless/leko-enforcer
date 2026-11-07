//! Judge the current and proposed documents before applying or parking a schedule change.
use crate::config_diff::item_diff::{diff_item, DiffContext};
use crate::config_diff::loosening_table::LooseningTable;
use crate::config_time::due_time::{due, effective};
use crate::config_time::zone_time::ZoneTime;
use crate::delayed_changes::switch_off_plan::{applied_locks, lock_words, request_moment, SwitchOffPlan};
use crate::domain::canonical::canonicalize;
use crate::domain::extended_schema::extended_schema;
use crate::edit_schedule::schedule_safety::is_schedule_loosening;
use crate::models::specs::{Lock, ReadSet};
use crate::sessions::utc_moment::format_utc_moment;
use serde_json::Value;

pub fn plan_schedule_edit(
    set: &ReadSet,
    id: &str,
    rule_id: &str,
    old_doc: &Value,
    new_doc: &Value,
    requested_at: i64,
    zone: &ZoneTime,
) -> Result<SwitchOffPlan, String> {
    let Some(locks) = applied_locks(set, id) else {
        return Ok(SwitchOffPlan::NoSuchBlock);
    };
    let requested_at = request_moment(requested_at);

    // Extract the specific rules
    let old_rules = old_doc.get("rules").and_then(Value::as_array).ok_or("Invalid old rules")?;
    let new_rules = new_doc.get("rules").and_then(Value::as_array).ok_or("Invalid new rules")?;
    let old_rule = old_rules.iter().find(|r| r.get("id").and_then(Value::as_str) == Some(rule_id))
        .ok_or_else(|| format!("Rule {rule_id:?} not found in old document"))?;
    let new_rule = new_rules.iter().find(|r| r.get("id").and_then(Value::as_str) == Some(rule_id))
        .ok_or_else(|| format!("Rule {rule_id:?} not found in new document"))?;

    // Safety check: must pass both the schema-based diff and the decision-based simulator
    let safety_loosens = is_schedule_loosening(old_rule, new_rule);

    let old_canon = canonicalize(extended_schema(), "block", old_doc)
        .map_err(|e| format!("Could not judge the existing block: {e}"))?;
    let new_canon = canonicalize(extended_schema(), "block", new_doc)
        .map_err(|e| format!("Could not judge the edited block: {e}"))?;
    let table = LooseningTable::new(extended_schema())
        .map_err(|e| format!("Could not prepare the loosening judge: {e}"))?;
    let changes = diff_item("block", &old_canon, &new_canon, &DiffContext { requested_g: requested_at, zone, table: &table });
    let diff_loosens = changes.iter().any(|change| change.loosens);

    if !safety_loosens && !diff_loosens {
        return Ok(SwitchOffPlan::LandsNow);
    }

    if locks.iter().any(|(_, lock)| lock.key().is_some()) {
        return Ok(SwitchOffPlan::NeedsPartnerKey);
    }
    let plain: Vec<_> = locks.iter().map(|(_, lock)| lock.clone()).collect();
    let Some(due_at) = due(requested_at, &plain, zone) else {
        return Ok(SwitchOffPlan::CannotLand);
    };
    if due_at <= requested_at {
        return Ok(SwitchOffPlan::LandsNow);
    }
    if due_at % 1000 != 0 || format_utc_moment(due_at).is_none() {
        return Ok(SwitchOffPlan::CannotLand);
    }
    let because = locks.iter().filter(|(_, lock)| !matches!(effective(lock, requested_at, zone), Lock::Open { .. }))
        .map(|(whose, lock)| format!("{whose} ({})", lock_words(lock))).collect();
    Ok(SwitchOffPlan::Waits { due_at, because })
}
