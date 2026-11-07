//! The waiting file's JSON: `{"format": 1, "changes": [...]}` with one record per parked change
//! (`kind`, `block`, `requestedAt`, `dueAt`). Format 2 supports `edit-coverage` with `operations`.
//! Format 3 supports `edit-schedule` with `rule`, `before`, and `after`.

use super::waiting_change::{moment_text, read_moment, ChangeKind, CoverageAction, CoverageOperation, WaitingChange};
use serde_json::{json, Map, Value};

pub const FORMAT: i64 = 3;
pub const MAX_CHANGES: usize = 1000;
const RECORD_FIELDS: [&str; 4] = ["kind", "block", "requestedAt", "dueAt"];

fn text_of<'a>(record: &'a Map<String, Value>, name: &str, place: &str) -> Result<&'a str, String> {
    record.get(name).and_then(Value::as_str).ok_or_else(|| format!("{place} needs \"{name}\" as text"))
}

fn read_record(record: &Value, number: usize, format: i64) -> Result<WaitingChange, String> {
    let place = format!("change {number}");
    let Some(record) = record.as_object() else { return Err(format!("{place} must be a JSON object")) };

    let word = text_of(record, "kind", &place)?;
    let Some(kind) = ChangeKind::from_word(word) else { return Err(format!("{place} has a kind {word:?} that this program does not know")) };

    for key in record.keys() {
        let allowed = match kind {
            ChangeKind::SwitchOff => RECORD_FIELDS.contains(&key.as_str()),
            ChangeKind::EditCoverage => RECORD_FIELDS.contains(&key.as_str()) || key == "operations",
            ChangeKind::EditSchedule => RECORD_FIELDS.contains(&key.as_str()) || key == "rule" || key == "before" || key == "after",
        };
        if !allowed {
            return Err(format!("{place} has a field \"{key}\" that this file does not have"));
        }
    }

    let block = text_of(record, "block", &place)?;
    let requested_at = read_moment(text_of(record, "requestedAt", &place)?, &format!("{place}: requestedAt"))?;
    let due_at = read_moment(text_of(record, "dueAt", &place)?, &format!("{place}: dueAt"))?;

    let result = match kind {
        ChangeKind::SwitchOff => {
            WaitingChange::new(kind, block, requested_at, due_at)
        }
        ChangeKind::EditCoverage => {
            if format < 2 { return Err("coverage edits require format 2 or higher".to_string()); }
            let values = record.get("operations").and_then(Value::as_array).ok_or_else(|| format!("{place} needs \"operations\" as a list"))?;
            if values.len() > super::waiting_change::MAX_EDIT_OPERATIONS { return Err(format!("{place} holds too many operations")); }
            let mut operations = Vec::with_capacity(values.len());
            for (index, value) in values.iter().enumerate() {
                let op_place = format!("{place}: operation {}", index + 1);
                let object = value.as_object().ok_or_else(|| format!("{op_place} must be a JSON object"))?;
                if let Some(key) = object.keys().find(|key| *key != "action" && *key != "target") { return Err(format!("{op_place} has an unknown field {key:?}")); }
                let word = text_of(object, "action", &op_place)?;
                let action = CoverageAction::from_word(word).ok_or_else(|| format!("{op_place} has an unknown action {word:?}"))?;
                operations.push(CoverageOperation { action, target: text_of(object, "target", &op_place)?.to_string() });
            }
            WaitingChange::edit(block, requested_at, due_at, operations)
        }
        ChangeKind::EditSchedule => {
            if format < 3 { return Err("schedule edits require format 3".to_string()); }
            let rule = text_of(record, "rule", &place)?;
            let before = record.get("before").cloned().ok_or_else(|| format!("{place} needs \"before\""))?;
            let after = record.get("after").cloned().ok_or_else(|| format!("{place} needs \"after\""))?;
            WaitingChange::schedule(block, rule, before, after, requested_at, due_at)
        }
    };
    result.map_err(|why| format!("{place}: {why}"))
}

/// The parked changes `text` holds, in the order written, or the reason it was refused.
pub fn read_waiting(text: &str) -> Result<Vec<WaitingChange>, String> {
    if text.chars().all(char::is_whitespace) {
        return Err("is empty".to_string());
    }
    let json: Value = serde_json::from_str(text).map_err(|_| "is not valid JSON".to_string())?;
    let Some(object) = json.as_object() else { return Err("must be a JSON object".to_string()) };
    if let Some(odd) = object.keys().find(|name| *name != "format" && *name != "changes") {
        return Err(format!("has a field \"{odd}\" that this file does not have"));
    }
    let format = match object.get("format").and_then(Value::as_i64) {
        Some(1 | 2 | FORMAT) => object["format"].as_i64().unwrap(),
        Some(newer) if newer > FORMAT => return Err(format!("is format {newer}, which is newer than the format {FORMAT} this program reads")),
        _ => return Err("needs \"format\": 1, 2 or 3".to_string()),
    };
    let Some(records) = object.get("changes").and_then(Value::as_array) else { return Err("needs \"changes\" as a list".to_string()) };
    if records.len() > MAX_CHANGES {
        return Err(format!("holds more than {MAX_CHANGES} changes"));
    }
    let mut changes: Vec<WaitingChange> = Vec::new();
    for (index, record) in records.iter().enumerate() {
        let change = read_record(record, index + 1, format)?;
        if changes.iter().any(|earlier| earlier.block == change.block) {
            return Err(format!("parks the block \"{}\" twice", change.block));
        }
        changes.push(change);
    }
    Ok(changes)
}

/// The text of the waiting file for `changes`, ending in a newline, or the sentence saying why it
/// cannot be written.
pub fn write_waiting(changes: &[WaitingChange]) -> Result<String, String> {
    if changes.len() > MAX_CHANGES { return Err(format!("holds more than {MAX_CHANGES} changes")); }
    let mut records = Vec::new();
    for (index, change) in changes.iter().enumerate() {
        if changes[..index].iter().any(|earlier| earlier.block == change.block) { return Err(format!("parks the block {:?} twice", change.block)); }
        let (Some(requested), Some(due)) = (moment_text(change.requested_at), moment_text(change.due_at)) else {
            return Err(format!("a moment of the change for \"{}\" cannot be written", change.block));
        };
        let mut record = json!({ "kind": change.kind.word(), "block": change.block, "requestedAt": requested, "dueAt": due });
        match change.kind {
            ChangeKind::SwitchOff => {
                WaitingChange::new(change.kind, &change.block, change.requested_at, change.due_at)?;
                if !change.operations.is_empty() { return Err("switch-off cannot carry operations".into()); }
            }
            ChangeKind::EditCoverage => {
                WaitingChange::edit(&change.block, change.requested_at, change.due_at, change.operations.clone())?;
                record["operations"] = json!(change.operations.iter().map(|op| json!({"action": op.action.word(), "target": op.target})).collect::<Vec<_>>());
            }
            ChangeKind::EditSchedule => {
                let rule = change.rule.as_deref().ok_or("missing rule id")?;
                let before = change.before.clone().ok_or("missing before precondition")?;
                let after = change.after.clone().ok_or("missing after replacement")?;
                WaitingChange::schedule(&change.block, rule, before.clone(), after.clone(), change.requested_at, change.due_at)?;
                record["rule"] = json!(rule);
                record["before"] = before;
                record["after"] = after;
            }
        }
        records.push(record);
    }
    let format = if changes.iter().any(|change| change.kind == ChangeKind::EditSchedule) {
        FORMAT
    } else if changes.iter().any(|change| change.kind == ChangeKind::EditCoverage) {
        2
    } else {
        1
    };
    let mut text = serde_json::to_string_pretty(&json!({ "format": format, "changes": records })).map_err(|problem| problem.to_string())?;
    text.push('\n');
    Ok(text)
}
