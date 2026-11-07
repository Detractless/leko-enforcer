//! Compares two versions of one rule, written as file JSON, and says whether the change loosens in
//! any part: a different kind, fewer weekdays, a bigger amount or count, a longer period, a scope
//! that widens or a wider set of allowed stretches. Ported from `lib/rule-compare.js`.

use crate::config::strict_fields::parse_minutes;
use crate::config_time::window_mask::WindowMask;
use crate::sessions::session_compare::session_loosens;
use serde_json::Value;
use std::collections::BTreeSet;

const PERIODS: [&str; 3] = ["hour", "day", "week"];
const ALL_DAYS: [&str; 7] = ["mon", "tue", "wed", "thu", "fri", "sat", "sun"];

/// The weekdays a rule or routine applies on; none named means all seven.
pub fn days_of(owner: &Value) -> BTreeSet<String> {
    let named: BTreeSet<String> = owner.get("days").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str).map(str::to_string).collect();
    if named.is_empty() {
        ALL_DAYS.iter().map(|day| day.to_string()).collect()
    } else {
        named
    }
}

fn text<'a>(rule: &'a Value, name: &str) -> Option<&'a str> {
    rule.get(name).and_then(Value::as_str)
}

fn number(rule: &Value, name: &str) -> f64 {
    rule.get(name).and_then(Value::as_f64).unwrap_or(0.0)
}

/// Where the rule's period stands in hour, day, week; -1 when it names none of them.
fn period_rank(rule: &Value) -> i64 {
    PERIODS.iter().position(|period| text(rule, "per") == Some(period)).map_or(-1, |at| at as i64)
}

fn longer(old: &Value, new: &Value) -> bool {
    period_rank(new) > period_rank(old)
}

fn scope_widens(old: &Value, new: &Value) -> bool {
    text(old, "scope") == Some("block") && text(new, "scope") == Some("app")
}

/// The minutes in which an outsideHours rule leaves the app open.
pub fn allowed_mask(rule: &Value) -> WindowMask {
    let minutes = |entry: &Value, name: &str| entry.get(name).and_then(Value::as_str).and_then(parse_minutes);
    let stretches: Vec<(i64, i64)> = rule
        .get("allowed")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|entry| Some((minutes(entry, "from")?, minutes(entry, "to")?)))
        .collect();
    WindowMask::of(&stretches)
}

/// True when `new` loosens `old`.
pub fn loosens(old: &Value, new: &Value) -> bool {
    let kind = text(old, "kind");
    if kind != text(new, "kind") {
        return true;
    }
    let new_days = days_of(new);
    if !days_of(old).iter().all(|day| new_days.contains(day)) {
        return true;
    }
    match kind {
        Some("minutes") => number(new, "amount") > number(old, "amount") || longer(old, new) || scope_widens(old, new),
        Some("opens") => number(new, "count") > number(old, "count") || longer(old, new) || scope_widens(old, new),
        Some("outsideHours") => !allowed_mask(new).subset_of(&allowed_mask(old)),
        Some("session") => session_loosens(old, new),
        _ => false,
    }
}
