//! Reads a settings zip all or nothing in the order the phone does: a 0.2 zip is recognised, a zip
//! without GUIDE.md or without a settings file is refused, an entry that appears twice or lies
//! outside the allowed places is refused naming it, then each settings file is read on its own,
//! and last the references between the items are checked. Ported from Enforcer's `lib/zip-check.js`.

use super::zip_reader::{open_zip, OpenedZip};
use crate::config::read_basics::read_basics;
use crate::config::read_block::{is_item_id, read_block, ReadContext};
use crate::config::read_deck::{read_book, read_deck};
use crate::config::read_practice::read_practice;
use crate::config::read_routine::read_routine;
use crate::config::strict_fields::{ConfigProblem, Read, StrictFields, WHOLE_FILE};
use crate::domain::set_rules::cross_problems;
use crate::models::specs::{ReadMode, ReadSet};
use serde_json::Value;
use std::collections::HashSet;

pub const GUIDE: &str = "GUIDE.md";
pub const SCHEMA_FILE: &str = "schema/enforcer-config-v2.schema.json";
pub const OLD_ZIP: &str = "This file was made by Enforcer 0.2. Export again from this version.";
const FOLDERS: [(&str, Kind); 5] = [("blocks", Kind::Block), ("practices", Kind::Practice), ("routines", Kind::Routine), ("decks", Kind::Deck), ("books", Kind::Book)];

/// The kind of a settings file.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Kind {
    General,
    Block,
    Practice,
    Routine,
    Deck,
    Book,
}

/// One problem of a zip: the entry it is about (none for the whole zip) and the sentence.
#[derive(Debug, Clone, PartialEq)]
pub struct Problem {
    pub entry: Option<String>,
    pub sentence: String,
}

/// A settings file of the zip and the problem found in it, if any.
#[derive(Debug, Clone, PartialEq)]
pub struct FileStatus {
    pub path: String,
    pub problem: Option<String>,
}

/// A zip the phone would read: every settings file, the set they make and its sorted listing.
#[derive(Debug, Clone)]
pub struct ReadZip {
    pub files: Vec<FileStatus>,
    pub set: ReadSet,
    pub listing: Vec<String>,
}

/// A zip the phone refuses whole: the first refusal and every problem found.
#[derive(Debug, Clone)]
pub struct Refusal {
    pub entry: Option<String>,
    pub sentence: String,
    pub problems: Vec<Problem>,
    pub files: Vec<FileStatus>,
}

#[derive(Debug, Clone)]
pub enum ZipVerdict {
    Read(ReadZip),
    Refused(Refusal),
}

/// The kind of settings file a zip path is, or `None` when it is not a settings file.
pub fn kind_of(path: &str) -> Option<Kind> {
    if path == "general.json" {
        return Some(Kind::General);
    }
    let (folder, kind) = FOLDERS.iter().find(|(name, _)| path.strip_prefix(name).is_some_and(|rest| rest.starts_with('/')))?;
    let stem = path[folder.len() + 1..].strip_suffix(".json")?;
    is_item_id(stem).then_some(*kind)
}

fn id_of(path: &str) -> &str {
    let name = &path[path.rfind('/').map_or(0, |at| at + 1)..];
    name.strip_suffix(".json").unwrap_or(name)
}

fn whole_file(reason: &str) -> ConfigProblem {
    ConfigProblem::new(WHOLE_FILE, reason)
}

fn parse_object(bytes: &[u8]) -> Result<Value, ConfigProblem> {
    let text = String::from_utf8_lossy(bytes);
    // The phone's parser reads an empty document as a null value, which is not an object.
    let blank = text.chars().all(|c| matches!(c, ' ' | '\t' | '\r' | '\n'));
    let value = if blank { Value::Null } else { serde_json::from_str(&text).map_err(|_| whole_file("is not valid JSON"))? };
    match value {
        Value::Object(_) => Ok(value),
        _ => Err(whole_file("must be a JSON object")),
    }
}

fn looks_like_old_deck(bytes: &[u8]) -> bool {
    match parse_object(bytes) {
        Ok(Value::Object(map)) => map.contains_key("schemaVersion") && !map.contains_key("schema"),
        _ => false,
    }
}

fn is_old_deck_path(name: &str) -> bool {
    name.strip_prefix("decks/").and_then(|rest| rest.strip_suffix(".json")).is_some_and(|stem| !stem.is_empty() && !stem.contains('/'))
}

fn is_zero_point_two(files: &[(String, Vec<u8>)]) -> bool {
    files.iter().any(|(name, bytes)| name.starts_with("config/") || (is_old_deck_path(name) && looks_like_old_deck(bytes)))
}

/// What one settings file read as.
enum Spec {
    Basics(crate::models::specs::Basics),
    Block(crate::models::specs::BlockSpec),
    Practice(crate::models::specs::PracticeSpec),
    Routine(crate::models::specs::RoutineSpec),
    Deck(crate::models::specs::ContentSpec),
    Book(crate::models::specs::ContentSpec),
}

fn read_spec(kind: Kind, fields: &mut StrictFields, context: &ReadContext) -> Read<Spec> {
    Ok(match kind {
        Kind::General => Spec::Basics(read_basics(fields, context)?),
        Kind::Block => Spec::Block(read_block(fields, context)?),
        Kind::Practice => Spec::Practice(read_practice(fields, context)?),
        Kind::Routine => Spec::Routine(read_routine(fields, context)?),
        Kind::Deck => Spec::Deck(read_deck(fields, context)?),
        Kind::Book => Spec::Book(read_book(fields, context)?),
    })
}

/// Reads one settings file on its own: the spec the reader returns and the parsed JSON.
fn read_settings_file(path: &str, kind: Kind, bytes: &[u8], always_allowed: &HashSet<String>, mode: ReadMode) -> Read<(Spec, Value)> {
    let json = parse_object(bytes)?;
    let spec = {
        let Value::Object(map) = &json else { return Err(whole_file("must be a JSON object")) };
        let mut fields = StrictFields::new(map, "").with_mode(mode);
        if !fields.is_stamped_with("schema", 2) {
            return fields.fail("schema", "is required and must be the number 2. Only format 2 is read");
        }
        read_spec(kind, &mut fields, &ReadContext { expected_id: Some(id_of(path)), always_allowed })?
    };
    Ok((spec, json))
}

fn build_set(read: Vec<(String, Spec, Value)>) -> ReadSet {
    let mut set = ReadSet::default();
    for (path, spec, json) in read {
        set.docs.push((path, json));
        match spec {
            Spec::Basics(basics) => set.basics = Some(basics),
            Spec::Block(item) => set.blocks.insert(item.id.clone(), item),
            Spec::Practice(item) => set.practices.insert(item.id.clone(), item),
            Spec::Routine(item) => set.routines.insert(item.id.clone(), item),
            Spec::Deck(item) => set.decks.insert(item.id.clone(), item),
            Spec::Book(item) => set.books.insert(item.id.clone(), item),
        }
    }
    set
}

/// The sorted names of the items in `set`: `general`, `block/id`, `practice/id`, and so on.
pub fn listing(set: &ReadSet) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    if set.basics.is_some() {
        names.push("general".to_string());
    }
    names.extend(set.blocks.items.iter().map(|(id, _)| format!("block/{id}")));
    names.extend(set.practices.items.iter().map(|(id, _)| format!("practice/{id}")));
    names.extend(set.routines.items.iter().map(|(id, _)| format!("routine/{id}")));
    names.extend(set.decks.items.iter().map(|(id, _)| format!("deck/{id}")));
    names.extend(set.books.items.iter().map(|(id, _)| format!("book/{id}")));
    names.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
    names
}

fn refusal(entry: Option<&str>, sentence: &str) -> Refusal {
    let entry = entry.map(str::to_string);
    Refusal { entry: entry.clone(), sentence: sentence.to_string(), problems: vec![Problem { entry, sentence: sentence.to_string() }], files: Vec::new() }
}

/// The structural refusals in the phone's order, or `None` when the zip's entries are acceptable.
fn structure(opened: &OpenedZip) -> Option<Refusal> {
    let names: Vec<&str> = opened.files.iter().map(|(name, _)| name.as_str()).collect();
    if is_zero_point_two(&opened.files) {
        return Some(refusal(None, OLD_ZIP));
    }
    if !names.contains(&GUIDE) {
        return Some(refusal(Some(GUIDE), "This zip has no GUIDE.md, so it was not made by Export in this app."));
    }
    if !names.iter().any(|name| kind_of(name).is_some()) {
        return Some(refusal(None, "This zip holds no settings file, so nothing was imported."));
    }
    let twice = opened.duplicates.iter().map(|name| (name.as_str(), format!("The entry \"{name}\" appears twice.")));
    let stray = names
        .iter()
        .filter(|name| **name != GUIDE && **name != SCHEMA_FILE && kind_of(name).is_none())
        .map(|name| (*name, format!("The entry \"{name}\" is not a place where Enforcer's settings files live.")));
    let problems: Vec<Problem> = twice.chain(stray).map(|(entry, sentence)| Problem { entry: Some(entry.to_string()), sentence }).collect();
    let first = problems.first()?;
    Some(Refusal { entry: first.entry.clone(), sentence: first.sentence.clone(), problems: problems.clone(), files: Vec::new() })
}

/// Checks the zip `bytes` as Leko Enforcer reads it. `always_allowed` is the set of packages
/// Enforcer never blocks.
pub fn check_zip(bytes: &[u8], always_allowed: &HashSet<String>) -> ZipVerdict {
    check_zip_in_mode(bytes, always_allowed, ReadMode::Extended)
}

/// Checks the zip `bytes` in `mode`: `Phone` reads exactly what the phone 0.3 reads, so a zip that
/// uses a newer field is refused the way the phone would refuse it.
pub fn check_zip_in_mode(bytes: &[u8], always_allowed: &HashSet<String>, mode: ReadMode) -> ZipVerdict {
    let opened = match open_zip(bytes) {
        Ok(opened) => opened,
        Err(reason) => return ZipVerdict::Refused(refusal(None, &reason)),
    };
    if let Some(stray) = structure(&opened) {
        return ZipVerdict::Refused(stray);
    }
    let mut read = Vec::new();
    let mut problems: Vec<Problem> = Vec::new();
    let mut files: Vec<FileStatus> = Vec::new();
    for (path, file_bytes) in &opened.files {
        let Some(kind) = kind_of(path) else { continue };
        let mut status = FileStatus { path: path.clone(), problem: None };
        match read_settings_file(path, kind, file_bytes, always_allowed, mode) {
            Ok((spec, json)) => read.push((path.clone(), spec, json)),
            Err(problem) => {
                let sentence = format!("{path}: {}", problem.sentence());
                problems.push(Problem { entry: Some(path.clone()), sentence: sentence.clone() });
                status.problem = Some(sentence);
            }
        }
        files.push(status);
    }
    let set = build_set(read);
    if problems.is_empty() {
        for found in cross_problems(&set) {
            let sentence = format!("{}: {}", found.entry, found.sentence());
            for file in files.iter_mut().filter(|file| file.path == found.entry) {
                file.problem.get_or_insert_with(|| sentence.clone());
            }
            problems.push(Problem { entry: Some(found.entry), sentence });
        }
    }
    if let Some(first) = problems.first() {
        return ZipVerdict::Refused(Refusal { entry: first.entry.clone(), sentence: first.sentence.clone(), problems: problems.clone(), files });
    }
    let listing = listing(&set);
    ZipVerdict::Read(ReadZip { files, set, listing })
}
