//! How a change of one field loosens. The directions are the `x-loosening` notes of the written
//! schema, which the phone's own test holds to the judge's table field by field, so reading the
//! notes here keeps one list. A field that is not listed is judged as a loosening.
//! Ported from `lib/loosening-table.js`.

use super::door_compare;
use crate::domain::schema_fields::all_fields;
use serde_json::Value;
use std::collections::HashMap;

/// The order of an enumerated value for the fields judged by up or down: a later value ranks higher.
const ENUM_ORDER: [(&str, [&str; 2]); 2] = [("block.pass.scope", ["app", "block"]), ("block.pass.risingCost", ["double", "none"])];

/// The fields where a value that is not there ranks above every number: a pass with no cap, a routine with no daily goal.
const HIGHEST_WHEN_ABSENT: [&str; 2] = ["block.pass.minutes", "routine.timesPerDay"];

/// A comparable rank: numbers by value, "unlimited" above every number, a removed pass cap above
/// every number, and a routine with no daily goal above every number.
fn rank(path: &str, value: Option<&Value>) -> f64 {
    match value {
        None | Some(Value::Null) => {
            if HIGHEST_WHEN_ABSENT.contains(&path) {
                f64::MAX
            } else {
                0.0
            }
        }
        Some(Value::Number(number)) => number.as_f64().unwrap_or(0.0),
        Some(other) => match ENUM_ORDER.iter().find(|(name, _)| *name == path) {
            Some((_, order)) => order.iter().position(|word| other.as_str() == Some(word)).map_or(-1.0, |at| at as f64),
            None if other.as_str() == Some("unlimited") => f64::MAX,
            None => 0.0,
        },
    }
}

/// The loosening direction of every field of the schema.
#[derive(Debug, Clone)]
pub struct LooseningTable {
    notes: HashMap<String, Option<String>>,
}

impl LooseningTable {
    /// The table for the schema `root`.
    pub fn new(root: &Value) -> Result<LooseningTable, String> {
        let mut notes: HashMap<String, Option<String>> = HashMap::new();
        for field in all_fields(root)? {
            notes.entry(field.path.replace("[]", "")).or_insert(field.loosening);
        }
        Ok(LooseningTable { notes })
    }

    /// The direction note of `path`; a field that is not listed is "any".
    pub fn of(&self, path: &str) -> &str {
        self.notes.get(path).and_then(Option::as_deref).unwrap_or("any")
    }

    /// True when changing the value at `path` from `old` to `new` loosens. `None` is a value that is not there.
    pub fn value_loosens(&self, path: &str, old: Option<&Value>, new: Option<&Value>) -> bool {
        match self.of(path) {
            "never" => false,
            "bool-off" => old == Some(&Value::Bool(true)) && new != Some(&Value::Bool(true)),
            "up" | "emergency" => rank(path, new) > rank(path, old),
            "down" => rank(path, new) < rank(path, old),
            "door" => door_compare::loosens(old, new),
            _ => true,
        }
    }
}
