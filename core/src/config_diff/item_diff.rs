//! The units that differ between two versions of an item written as canonical file JSON, each with
//! the direction of the change. [`DiffContext`] holds the moment of the import (`requested_g`, in
//! milliseconds), the zone and the loosening table. Ported from `lib/item-diff.js`.

use super::diff_parts::{elements, keyed, texts, unit, value, Unit};
use super::loosening_table::LooseningTable;
use super::lock_compare::{level_tightens, same_level};
use super::rule_compare;
use super::small_item_diff::small_item_diff;
use crate::config::read_key_lock::{read_lock, MAX_HOURS};
use crate::config::strict_fields::StrictFields;
use crate::config_time::zone_time::ZoneTime;
use crate::domain::json_same::same_json_opt;
use crate::models::specs::Lock;
use crate::sessions::session_compare::rules_in_force;
use serde_json::{Map, Value};

pub struct DiffContext<'a> {
    pub requested_g: i64,
    pub zone: &'a ZoneTime,
    pub table: &'a LooseningTable,
}

/// The lock written in `item`; a lock that cannot be read counts as the longest wait.
pub fn lock_of(item: &Value) -> Lock {
    let read = item.as_object().map(|object| read_lock(&mut StrictFields::new(object, "")));
    match read {
        Some(Ok(lock)) => lock,
        _ => Lock::Wait { hours: MAX_HOURS, key: None },
    }
}

fn without_key(lock: Option<&Value>) -> Option<Value> {
    let mut rest: Map<String, Value> = lock?.as_object()?.clone();
    rest.shift_remove("partnerKeyHash");
    Some(Value::Object(rest))
}

fn lock_units(out: &mut Vec<Unit>, prefix: &str, old: &Value, new: &Value, context: &DiffContext) {
    let (old_lock, new_lock) = (lock_of(old), lock_of(new));
    if !same_level(&old_lock, &new_lock) {
        let tightens = level_tightens(&old_lock, &new_lock, context.requested_g, context.zone);
        out.push(unit("lock", &format!("{prefix}.lock"), without_key(old.get("lock")), without_key(new.get("lock")), !tightens));
    }
    let key_of = |item: &Value| item.get("lock").and_then(|lock| lock.get("partnerKeyHash")).cloned();
    let (old_key, new_key) = (key_of(old), key_of(new));
    if !same_json_opt(old_key.as_ref(), new_key.as_ref()) {
        out.push(unit("lock.partnerKeyHash", &format!("{prefix}.lock.partnerKeyHash"), old_key, new_key.clone(), new_key.is_some()));
    }
}

/// How a block names the apps it covers. A covers object that holds sites and no app list names none.
#[derive(Clone, Copy, PartialEq)]
enum AppForm {
    Apps,
    AllExcept,
    Nothing,
}

fn app_form(covers: Option<&Value>) -> AppForm {
    let holds = |key: &str| covers.and_then(Value::as_object).is_some_and(|object| object.contains_key(key));
    if holds("apps") {
        AppForm::Apps
    } else if holds("sites") && !holds("allAppsExcept") {
        AppForm::Nothing
    } else {
        AppForm::AllExcept
    }
}

/// True when the new covers is an all-except list that leaves out none of the apps the old list named.
fn covers_everything(old: &Value, new: &Value) -> bool {
    if app_form(old.get("covers")) != AppForm::Apps {
        return false;
    }
    let except = texts(new, "covers.allAppsExcept");
    texts(old, "covers.apps").iter().all(|name| !except.contains(name))
}

/// The covers value without its sites, which are compared one by one.
fn without_sites(covers: Option<&Value>) -> Option<Value> {
    let mut rest = covers?.clone();
    if let Value::Object(object) = &mut rest {
        object.shift_remove("sites");
    }
    Some(rest)
}

fn covers_units(out: &mut Vec<Unit>, table: &LooseningTable, old: &Value, new: &Value) {
    let (was, now) = (app_form(old.get("covers")), app_form(new.get("covers")));
    if was != now {
        // Dropping the app coverage loosens, adding it tightens, otherwise the two lists are compared.
        let loosens = match (was, now) {
            (_, AppForm::Nothing) => true,
            (AppForm::Nothing, _) => false,
            _ => !covers_everything(old, new),
        };
        out.push(unit("covers", "block.covers", without_sites(old.get("covers")), without_sites(new.get("covers")), loosens));
    } else if was != AppForm::Nothing {
        let path = if was == AppForm::Apps { "covers.apps" } else { "covers.allAppsExcept" };
        elements(out, table, "block", path, &texts(old, path), &texts(new, path));
    }
    elements(out, table, "block", "covers.sites", &texts(old, "covers.sites"), &texts(new, "covers.sites"));
}

fn section_units(out: &mut Vec<Unit>, table: &LooseningTable, name: &str, fields: &[&str], old: &Value, new: &Value) {
    let (was, now) = (old.get(name), new.get(name));
    let schema_path = format!("block.{name}");
    match (was, now) {
        (None, None) => {}
        (None, Some(now)) => out.push(unit(name, &schema_path, None, Some(now.clone()), true)),
        (Some(was), None) => out.push(unit(name, &schema_path, Some(was.clone()), None, false)),
        (Some(was), Some(now)) if name == "pass" && !same_json_opt(was.get("length"), now.get("length")) => {
            out.push(unit("pass", "block.pass.length", Some(was.clone()), Some(now.clone()), true));
        }
        _ => fields.iter().for_each(|field| value(out, table, "block", &format!("{name}.{field}"), old, new)),
    }
}

fn block_units(out: &mut Vec<Unit>, table: &LooseningTable, requested_g: i64, old: &Value, new: &Value) {
    for name in ["name", "on", "message"] {
        value(out, table, "block", name, old, new);
    }
    covers_units(out, table, old, new);
    keyed(out, "rules", "block.rules", &rules_in_force(old, requested_g), &rules_in_force(new, requested_g), rule_compare::loosens);
    value(out, table, "block", "door", old, new);
    section_units(out, table, "pass", &["minutes", "scope", "perDay", "risingCost"], old, new);
    section_units(out, table, "earn", &["practice", "minutesEach", "maxPerDay"], old, new);
}

/// The units that differ between `old` and `new`, two canonical documents of `kind`.
pub fn diff_item(kind: &str, old: &Value, new: &Value, context: &DiffContext) -> Vec<Unit> {
    let mut out = Vec::new();
    if kind == "block" {
        block_units(&mut out, context.table, context.requested_g, old, new);
    } else {
        small_item_diff(&mut out, context.table, kind, old, new);
    }
    lock_units(&mut out, kind, old, new, context);
    out
}
