//! The small steps the item diffs share. A unit of change is a [`Unit`]: `path` names it inside the
//! item ("pass.perDay", "rules#night", "covers.apps#com.example"), `schema_path` names the field in
//! the loosening table, and `old` or `new` is `None` where the unit was absent.
//! Ported from `lib/diff-parts.js`.

use super::loosening_table::LooseningTable;
use crate::domain::json_same::{same_json, same_json_opt};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq)]
pub struct Unit {
    pub path: String,
    pub schema_path: String,
    pub old: Option<Value>,
    pub new: Option<Value>,
    pub loosens: bool,
}

/// The value at a dotted path such as "pass.perDay", or `None` when any step is absent.
pub fn at<'a>(root: &'a Value, path: &str) -> Option<&'a Value> {
    let mut current = root;
    for step in path.split('.') {
        current = current.as_object()?.get(step)?;
    }
    Some(current)
}

pub fn unit(path: &str, schema_path: &str, old: Option<Value>, new: Option<Value>, loosens: bool) -> Unit {
    Unit { path: path.to_string(), schema_path: schema_path.to_string(), old, new, loosens }
}

/// Adds a unit for the value at `path` when it differs.
pub fn value(out: &mut Vec<Unit>, table: &LooseningTable, prefix: &str, path: &str, old: &Value, new: &Value) {
    let (before, after) = (at(old, path), at(new, path));
    if same_json_opt(before, after) {
        return;
    }
    let schema_path = format!("{prefix}.{path}");
    out.push(unit(path, &schema_path, before.cloned(), after.cloned(), table.value_loosens(&schema_path, before, after)));
}

/// The members of a list of texts found at `path`, in order; empty when absent.
pub fn texts(root: &Value, path: &str) -> Vec<String> {
    at(root, path).and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str).map(str::to_string).collect()
}

/// The members of a list of numbers found at `path`, in order, as the texts they print; empty when absent.
pub fn number_texts(root: &Value, path: &str) -> Vec<String> {
    let print = |number: &serde_json::Number| number.as_i64().map_or_else(|| number.as_f64().unwrap_or(0.0).to_string(), |whole| whole.to_string());
    at(root, path).and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_number).map(print).collect()
}

/// Adds one unit for each element that joined or left a list of texts at `path`. For set-join an element leaving tightens.
pub fn elements(out: &mut Vec<Unit>, table: &LooseningTable, prefix: &str, path: &str, old: &[String], new: &[String]) {
    let schema_path = format!("{prefix}.{path}");
    let leave_loosens = table.of(&schema_path) != "set-join";
    for left in old.iter().filter(|item| !new.contains(item)) {
        out.push(unit(&format!("{path}#{left}"), &schema_path, Some(Value::from(left.as_str())), None, leave_loosens));
    }
    for joined in new.iter().filter(|item| !old.contains(item)) {
        out.push(unit(&format!("{path}#{joined}"), &schema_path, None, Some(Value::from(joined.as_str())), !leave_loosens));
    }
}

/// The objects of a list keyed by their "id", in order of first appearance.
pub fn by_id<'a>(root: &'a Value, path: &str) -> Vec<(String, &'a Value)> {
    let mut found: Vec<(String, &Value)> = Vec::new();
    for item in at(root, path).and_then(Value::as_array).into_iter().flatten() {
        let Some(id) = item.as_object().and_then(|object| object.get("id")).and_then(Value::as_str) else { continue };
        match found.iter_mut().find(|(name, _)| name == id) {
            Some(entry) => entry.1 = item,
            None => found.push((id.to_string(), item)),
        }
    }
    found
}

fn find<'a>(list: &[(String, &'a Value)], id: &str) -> Option<&'a Value> {
    list.iter().find(|(name, _)| name == id).map(|(_, item)| *item)
}

/// Adds a unit for each id that appeared, left or changed between two keyed lists; `changed` says whether a change loosens.
pub fn keyed(out: &mut Vec<Unit>, path: &str, schema_path: &str, before: &[(String, &Value)], after: &[(String, &Value)], changed: fn(&Value, &Value) -> bool) {
    let ids = before.iter().map(|(id, _)| id).chain(after.iter().map(|(id, _)| id).filter(|id| find(before, id).is_none()));
    for id in ids {
        let unit_path = format!("{path}#{id}");
        match (find(before, id), find(after, id)) {
            (None, Some(now)) => out.push(unit(&unit_path, schema_path, None, Some(now.clone()), false)),
            (Some(was), None) => out.push(unit(&unit_path, schema_path, Some(was.clone()), None, true)),
            (Some(was), Some(now)) if !same_json(was, now) => out.push(unit(&unit_path, schema_path, Some(was.clone()), Some(now.clone()), changed(was, now))),
            _ => {}
        }
    }
}
