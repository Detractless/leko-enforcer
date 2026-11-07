//! Compares two versions of a door, written as file JSON. Only a wall outranks the other kinds; a
//! walking door asks more at a higher pace. Ported from `lib/door-compare.js`.

use crate::config::strict_fields::utf16_len;
use crate::domain::json_same::same_json;
use serde_json::Value;

/// A walking door with no pace written asks for the default, 100 a minute.
const DEFAULT_PACE: f64 = 100.0;

fn text<'a>(door: &'a Value, name: &str) -> Option<&'a str> {
    door.get(name).and_then(Value::as_str)
}

fn number(door: &Value, name: &str) -> f64 {
    door.get(name).and_then(Value::as_f64).unwrap_or(0.0)
}

fn pace(door: &Value) -> f64 {
    door.get("stepsPerMinute").and_then(Value::as_f64).unwrap_or(DEFAULT_PACE)
}

fn is_longer_version(old: &str, new: &str) -> bool {
    utf16_len(new) > utf16_len(old) && new.starts_with(old)
}

/// True when changing the door from `old` to `new` loosens; two equal doors are no change and do
/// not. A door that is not there, or not an object, counts as a loosening.
pub fn loosens(old: Option<&Value>, new: Option<&Value>) -> bool {
    let (Some(old), Some(new)) = (old.filter(|door| door.is_object()), new.filter(|door| door.is_object())) else { return true };
    if same_json(old, new) {
        return false;
    }
    let (Some(old_kind), Some(new_kind)) = (text(old, "kind"), text(new, "kind")) else { return true };
    if old_kind != new_kind {
        return old_kind == "wall" || new_kind != "wall";
    }
    match old_kind {
        "wait" => number(new, "seconds") < number(old, "seconds"),
        "walking" => pace(new) < pace(old),
        "sentence" => !is_longer_version(text(old, "sentence").unwrap_or(""), text(new, "sentence").unwrap_or("")),
        _ => true,
    }
}
