//! The rules a zip cannot show on its own, judged against the phone's last export as the phone
//! judges them against its stored state, in the phone's order: first the key policy (a key in the
//! old unsalted form is accepted only when the phone already holds the same key), then uninstall
//! protection (while it is on, a file may not change the mode, lower the days or change the key,
//! and may raise the days; a file with no protection block leaves it alone). Ported from
//! Enforcer's `lib/stored-state.js`.

use crate::models::specs::{Key, Protection, ReadSet};
use std::collections::HashSet;

/// What the phone does with a set judged against its stored state.
#[derive(Debug, Clone, PartialEq)]
pub enum StoredVerdict {
    Accepted,
    Refused(String),
}

/// How a file's protection block differs from the stored one.
#[derive(Debug, Clone, PartialEq)]
pub struct ProtectionChange {
    pub field: &'static str,
    pub weakens: bool,
}

/// The protection a phone has when its export names none: a wait of one day.
pub static DEFAULT_PROTECTION: Protection = Protection::Wait { days: 1 };

type KeyIdentity<'a> = (&'a str, Option<i64>, Option<&'a str>, &'a str);

fn identity(key: &Key) -> KeyIdentity<'_> {
    (&key.alg, key.iterations, key.salt.as_deref(), &key.hash)
}

/// Every key a set holds with the entry that holds it, in the phone's order: blocks, practices,
/// routines, decks, books, then the general file.
pub fn keys_in(set: &ReadSet) -> Vec<(String, &Key)> {
    let mut found: Vec<(String, &Key)> = Vec::new();
    for block in set.blocks.values() {
        let entry = format!("blocks/{}.json", block.id);
        if let crate::models::specs::Door::Key { key } = &block.door {
            found.push((entry.clone(), key));
        }
        found.extend(block.lock.key().map(|key| (entry.clone(), key)));
    }
    let locks = [
        ("practices", set.practices.values().map(|item| (item.id.as_str(), &item.lock)).collect::<Vec<_>>()),
        ("routines", set.routines.values().map(|item| (item.id.as_str(), &item.lock)).collect()),
        ("decks", set.decks.values().map(|item| (item.id.as_str(), &item.lock)).collect()),
        ("books", set.books.values().map(|item| (item.id.as_str(), &item.lock)).collect()),
    ];
    for (folder, items) in locks {
        found.extend(items.into_iter().filter_map(|(id, lock)| lock.key().map(|key| (format!("{folder}/{id}.json"), key))));
    }
    if let Some(basics) = &set.basics {
        found.extend(basics.lock.key().map(|key| ("general.json".to_string(), key)));
        if let Some(Protection::Key { key }) = &basics.protection {
            found.push(("general.json".to_string(), key));
        }
    }
    found
}

fn key_problem(stored: &ReadSet, incoming: &ReadSet) -> Option<String> {
    let held: HashSet<KeyIdentity> = keys_in(stored).into_iter().map(|(_, key)| identity(key)).collect();
    let (entry, _) = keys_in(incoming).into_iter().find(|(_, key)| key.alg == "sha256" && !held.contains(&identity(key)))?;
    Some(format!("{entry} holds a key in the old unsalted form, which a file may not introduce."))
}

/// The field a file's protection block weakens against `stored`, if it weakens one.
fn weakens(stored: &Protection, file: &Protection) -> Option<&'static str> {
    match (stored, file) {
        (Protection::Key { key: held }, Protection::Key { key }) => (key != held).then_some("uninstallProtection.keyHash"),
        (Protection::Wait { days: held }, Protection::Wait { days }) => (days < held).then_some("uninstallProtection.days"),
        _ => Some("uninstallProtection.mode"),
    }
}

/// What a file's uninstall protection block changes against `was` (the block of the last export,
/// or the default of one day): `None` when it changes nothing, a change that does not weaken when
/// it only raises the days, and a weakening otherwise.
pub fn protection_change(was: &Protection, file: &Protection) -> Option<ProtectionChange> {
    if let Some(field) = weakens(was, file) {
        return Some(ProtectionChange { field, weakens: true });
    }
    match (was, file) {
        (Protection::Wait { days: held }, Protection::Wait { days }) if days > held => Some(ProtectionChange { field: "uninstallProtection.days", weakens: false }),
        _ => None,
    }
}

/// Judges `incoming` against `stored` (the set read from the last export, or `None` when there is
/// none: then only the key rule can be judged, with no key held). `protection_is_on` is the
/// reading of whether uninstall protection is on.
pub fn judge_stored_state(incoming: &ReadSet, stored: Option<&ReadSet>, protection_is_on: bool) -> StoredVerdict {
    let empty = ReadSet::default();
    if let Some(sentence) = key_problem(stored.unwrap_or(&empty), incoming) {
        return StoredVerdict::Refused(sentence);
    }
    let Some(stored) = stored else {
        return StoredVerdict::Accepted;
    };
    let file = incoming.basics.as_ref().and_then(|basics| basics.protection.as_ref());
    let was = stored.basics.as_ref().and_then(|basics| basics.protection.as_ref()).unwrap_or(&DEFAULT_PROTECTION);
    match file.filter(|_| protection_is_on).and_then(|file| weakens(was, file)) {
        Some(field) => StoredVerdict::Refused(format!("Uninstall protection is on, so \"{field}\" cannot be weakened by a file.")),
        None => StoredVerdict::Accepted,
    }
}
