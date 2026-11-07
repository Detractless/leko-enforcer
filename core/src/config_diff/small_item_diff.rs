//! The units that differ between two versions of a practice, a routine, a deck, a book or the
//! general file, in canonical file JSON. The lock is added by the caller (`item_diff`).
//! Ported from `lib/small-item-diff.js`.

use super::diff_parts::{at, by_id, elements, keyed, number_texts, texts, unit, value, Unit};
use super::loosening_table::LooseningTable;
use super::rule_compare::days_of;
use crate::config::strict_fields::parse_minutes;
use crate::config_time::window_mask::WindowMask;
use crate::domain::json_same::same_json_opt;
use serde_json::{Map, Value};

const PRACTICE_FIELDS: [&str; 11] = ["name", "activity", "sets", "amountPerSet", "restSeconds", "deck", "book", "match", "parts", "verify", "typing"];
const BOOK_FIELDS: [&str; 5] = ["name", "text", "wordsPerPage", "atEnd", "resetToken"];
const GENERAL_FIELDS: [&str; 8] = ["dayStart", "theme", "tracking", "countdownBubble", "emergency.perDay", "emergency.minutes", "practiceRecording", "repSound"];

fn is_times_form(routine: &Value) -> bool {
    routine.get("due").and_then(Value::as_object).is_some_and(|due| due.contains_key("times"))
}

/// The routine's window in minutes after midnight; a bound that is not there or not a time is 0.
fn window(routine: &Value) -> (i64, i64) {
    let minutes = |name: &str| at(routine, &format!("due.{name}")).and_then(Value::as_str).and_then(parse_minutes).unwrap_or(0);
    (minutes("from"), minutes("to"))
}

fn window_json(routine: &Value) -> Value {
    let mut bounds = Map::new();
    for name in ["from", "to"] {
        if let Some(bound) = at(routine, &format!("due.{name}")) {
            bounds.insert(name.to_string(), bound.clone());
        }
    }
    Value::Object(bounds)
}

/// From and to are one unit: the window loosens when it no longer covers a minute it covered before.
fn window_unit(out: &mut Vec<Unit>, old: &Value, new: &Value) {
    let (was, now) = (window(old), window(new));
    if was == now {
        return;
    }
    let covers = WindowMask::of(&[was]).subset_of(&WindowMask::of(&[now]));
    out.push(unit("due.window", "routine.due.from", Some(window_json(old)), Some(window_json(new)), !covers));
}

fn sorted_days(routine: &Value) -> Vec<String> {
    days_of(routine.get("due").filter(|due| due.is_object()).unwrap_or(&Value::Null)).into_iter().collect()
}

fn due_units(out: &mut Vec<Unit>, table: &LooseningTable, old: &Value, new: &Value) {
    if is_times_form(old) != is_times_form(new) {
        if !same_json_opt(old.get("due"), new.get("due")) {
            out.push(unit("due", "routine.due", old.get("due").cloned(), new.get("due").cloned(), true));
        }
        return;
    }
    if is_times_form(old) {
        elements(out, table, "routine", "due.times", &texts(old, "due.times"), &texts(new, "due.times"));
    } else {
        value(out, table, "routine", "due.everyMinutes", old, new);
        window_unit(out, old, new);
    }
    elements(out, table, "routine", "due.days", &sorted_days(old), &sorted_days(new));
}

fn routine(out: &mut Vec<Unit>, table: &LooseningTable, old: &Value, new: &Value) {
    for name in ["name", "on", "practice"] {
        value(out, table, "routine", name, old, new);
    }
    due_units(out, table, old, new);
    elements(out, table, "routine", "ifSkipped.holdBlocks", &texts(old, "ifSkipped.holdBlocks"), &texts(new, "ifSkipped.holdBlocks"));
    // A snooze's lengths are numbers, read as the texts they print (the phone's judge does the same), one unit for each that joined or left.
    elements(out, table, "routine", "ifSkipped.snooze.minutes", &number_texts(old, "ifSkipped.snooze.minutes"), &number_texts(new, "ifSkipped.snooze.minutes"));
    for name in ["ifSkipped.snooze.perDay", "timesPerDay", "ifSkipped.takeOver", "ifSkipped.ring"] {
        value(out, table, "routine", name, old, new);
    }
}

fn deck(out: &mut Vec<Unit>, table: &LooseningTable, old: &Value, new: &Value) {
    for name in ["name", "returnRule.kind", "returnRule.restHours", "resetToken"] {
        value(out, table, "deck", name, old, new);
    }
    keyed(out, "cards", "deck.cards", &by_id(old, "cards"), &by_id(new, "cards"), |_, _| true);
}

/// Adds the units of a practice, a routine, a deck, a book or the general file to `out`.
pub fn small_item_diff(out: &mut Vec<Unit>, table: &LooseningTable, kind: &str, old: &Value, new: &Value) {
    match kind {
        "practice" => PRACTICE_FIELDS.iter().for_each(|name| value(out, table, "practice", name, old, new)),
        "routine" => routine(out, table, old, new),
        "deck" => deck(out, table, old, new),
        "book" => BOOK_FIELDS.iter().for_each(|name| value(out, table, "book", name, old, new)),
        "general" => {
            GENERAL_FIELDS.iter().for_each(|name| value(out, table, "general", name, old, new));
            elements(out, table, "general", "neverTakeOver", &texts(old, "neverTakeOver"), &texts(new, "neverTakeOver"));
        }
        _ => {}
    }
}
