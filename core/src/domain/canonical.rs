//! A settings document in canonical form: every default written, the way the phone reads an item
//! and writes it back before it compares two versions. The defaults come from the `default`
//! keyword of the written schema and nowhere else; the code adds only the normalisations the
//! phone's write makes (weekday lists, repeated names, the pass scope, the lock window's hours, the
//! rest hours of a deck's return rule, the lengths of a snooze, and a practice of parts, which
//! holds neither sets nor rest). Ported from `lib/canonical.js`.

use super::json_same::same_json;
use super::schema_fields::{definition, resolve_node};
use serde_json::{Map, Value};

const WEEKDAYS: [&str; 7] = ["mon", "tue", "wed", "thu", "fri", "sat", "sun"];
const WALKING_PACE: i64 = 100;

type Doc = Map<String, Value>;

fn fill_defaults(root: &Value, declared: &Value, value: &mut Value) -> Result<(), String> {
    let node = resolve_node(root, declared)?;
    let properties = node.get("properties").and_then(Value::as_object);
    match value {
        Value::Object(map) if properties.is_some() => {
            for (name, child) in properties.into_iter().flatten() {
                if !map.contains_key(name) {
                    if let Some(default) = resolve_node(root, child)?.get("default") {
                        map.insert(name.clone(), default.clone());
                    }
                }
                if let Some(inner) = map.get_mut(name) {
                    fill_defaults(root, child, inner)?;
                }
            }
        }
        Value::Array(list) if node.get("items").is_some_and(|items| !items.is_null()) => {
            for element in list {
                fill_defaults(root, &node["items"], element)?;
            }
        }
        _ => {}
    }
    Ok(())
}

/// Weekday lists in weekday order; naming none or all seven means every day, which is written by leaving the list out.
fn normalise_days(owner: &mut Doc) {
    let named: Vec<Value> = match owner.get("days") {
        Some(Value::Array(days)) => WEEKDAYS.iter().filter(|day| days.iter().any(|named| named.as_str() == Some(**day))).map(|day| Value::from(*day)).collect(),
        _ => return,
    };
    if named.is_empty() || named.len() == WEEKDAYS.len() {
        owner.shift_remove("days");
    } else {
        owner.insert("days".to_string(), Value::Array(named));
    }
}

/// A list with each repeated name or number kept once, in order. Lists and objects are never repeats of each other.
fn distinct_of(list: &[Value]) -> Vec<Value> {
    let mut kept: Vec<Value> = Vec::new();
    for item in list {
        let scalar = !item.is_object() && !item.is_array();
        if !(scalar && kept.iter().any(|seen| same_json(seen, item))) {
            kept.push(item.clone());
        }
    }
    kept
}

fn make_distinct(owner: &mut Doc, key: &str) {
    if let Some(Value::Array(list)) = owner.get(key) {
        let kept = distinct_of(list);
        owner.insert(key.to_string(), Value::Array(kept));
    }
}

fn normalise_lock(lock: &mut Value) {
    if let Value::Object(lock) = lock {
        if lock.get("level").and_then(Value::as_str) == Some("changeHours") && !lock.contains_key("hours") {
            lock.insert("hours".to_string(), Value::from(0));
        }
    }
}

fn normalise_pass(pass: &mut Value) {
    let Value::Object(pass) = pass else { return };
    if pass.get("length").and_then(Value::as_str) == Some("oneOpen") {
        pass.shift_remove("scope");
    } else if !pass.contains_key("scope") {
        pass.insert("scope".to_string(), Value::from("block"));
    }
}

fn normalise_block(doc: &mut Doc) {
    if doc.get("message").and_then(Value::as_str) == Some("") {
        doc.shift_remove("message");
    }
    if let Some(Value::Object(covers)) = doc.get_mut("covers") {
        make_distinct(covers, "apps");
        make_distinct(covers, "allAppsExcept");
        make_distinct(covers, "sites");
    }
    if let Some(Value::Array(rules)) = doc.get_mut("rules") {
        rules.iter_mut().filter_map(Value::as_object_mut).for_each(normalise_days);
    }
    if let Some(pass) = doc.get_mut("pass") {
        normalise_pass(pass);
    }
    // The phone writes a walking door's pace always, 100 when the file left it out.
    if let Some(Value::Object(door)) = doc.get_mut("door") {
        if door.get("kind").and_then(Value::as_str) == Some("walking") && !door.contains_key("stepsPerMinute") {
            door.insert("stepsPerMinute".to_string(), Value::from(WALKING_PACE));
        }
    }
}

/// The text a script's default sort compares: the UTF-16 units of the name.
fn sort_key(value: &Value) -> Vec<u16> {
    value.as_str().map_or_else(|| value.to_string(), str::to_string).encode_utf16().collect()
}

fn normalise_snooze(snooze: &mut Value) {
    let Value::Object(snooze) = snooze else { return };
    let Some(Value::Array(minutes)) = snooze.get("minutes") else { return };
    if !minutes.iter().all(Value::is_number) {
        return;
    }
    let mut kept = distinct_of(minutes);
    kept.sort_by(|a, b| a.as_f64().partial_cmp(&b.as_f64()).unwrap_or(std::cmp::Ordering::Equal));
    snooze.insert("minutes".to_string(), Value::Array(kept));
}

fn normalise_routine(doc: &mut Doc) {
    if let Some(Value::Object(due)) = doc.get_mut("due") {
        normalise_days(due);
        if let Some(Value::Array(times)) = due.get("times") {
            let mut kept = distinct_of(times);
            kept.sort_by_key(sort_key);
            due.insert("times".to_string(), Value::Array(kept));
        }
    }
    if let Some(Value::Object(skipped)) = doc.get_mut("ifSkipped") {
        make_distinct(skipped, "holdBlocks");
        // The phone writes the ring only when it is on, so a written false is the same file as one that leaves it out.
        if skipped.get("ring") == Some(&Value::Bool(false)) {
            skipped.shift_remove("ring");
        }
        if let Some(snooze) = skipped.get_mut("snooze") {
            normalise_snooze(snooze);
        }
    }
}

fn normalise_deck(doc: &mut Doc) {
    if let Some(Value::Object(rule)) = doc.get_mut("returnRule") {
        if !rule.contains_key("restHours") {
            let rest = if rule.get("kind").and_then(Value::as_str) == Some("wholeDeck") { 0 } else { 24 };
            rule.insert("restHours".to_string(), Value::from(rest));
        }
    }
}

// A walk is one set with no rest, and a practice of parts is one set of each part with no rest;
// the phone writes neither field for either. Only a reading practice writes its match.
fn normalise_practice(doc: &mut Doc) {
    let activity = doc.get("activity").and_then(Value::as_str).map(str::to_string);
    if activity.as_deref() != Some("readAloud") {
        doc.shift_remove("match");
    }
    if activity.as_deref() == Some("walking") || doc.contains_key("parts") {
        doc.shift_remove("sets");
        doc.shift_remove("restSeconds");
    }
}

// The phone writes the recording and the ding switches only when they are off, so a written true
// is the same file as one that leaves them out.
fn normalise_general(doc: &mut Doc) {
    for key in ["practiceRecording", "repSound"] {
        if doc.get(key) == Some(&Value::Bool(true)) {
            doc.shift_remove(key);
        }
    }
}

/// The canonical form of `doc`, a file of `kind` ("general", "block", "practice", "routine", "deck"
/// or "book") judged against the schema `root`. `doc` is not changed; anything that is not an
/// object comes back as it is.
pub fn canonicalize(root: &Value, kind: &str, doc: &Value) -> Result<Value, String> {
    if !doc.is_object() {
        return Ok(doc.clone());
    }
    let mut copy = doc.clone();
    fill_defaults(root, definition(root, kind)?, &mut copy)?;
    if let Value::Object(map) = &mut copy {
        match kind {
            "general" => normalise_general(map),
            "block" => normalise_block(map),
            "practice" => normalise_practice(map),
            "routine" => normalise_routine(map),
            "deck" => normalise_deck(map),
            _ => {}
        }
        if let Some(lock) = map.get_mut("lock") {
            normalise_lock(lock);
        }
    }
    Ok(copy)
}
