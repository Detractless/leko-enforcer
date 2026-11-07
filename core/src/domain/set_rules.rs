//! Checks the references between the items of one zip, as the phone does once every file has been
//! read on its own. Ported from Enforcer's `lib/set-rules.js`.

use crate::models::specs::{Door, ReadSet};
use std::collections::HashSet;

/// One reference that does not hold: the zip path, the dotted field and the reason.
#[derive(Debug, Clone, PartialEq)]
pub struct CrossProblem {
    pub entry: String,
    pub field: String,
    pub reason: String,
}

impl CrossProblem {
    fn new(entry: &str, field: &str, reason: String) -> CrossProblem {
        CrossProblem { entry: entry.to_string(), field: field.to_string(), reason }
    }

    pub fn sentence(&self) -> String {
        format!("\"{}\" {}", self.field, self.reason)
    }
}

fn missing(what: &str, name: &str) -> String {
    format!("names the {what} \"{name}\", which is not in this set")
}

fn block_problems(set: &ReadSet, held_by: &HashSet<&str>) -> Vec<CrossProblem> {
    let mut found = Vec::new();
    for block in set.blocks.values() {
        let entry = format!("blocks/{}.json", block.id);
        if let Door::Practice { practice } = &block.door {
            if !set.practices.contains(practice) {
                found.push(CrossProblem::new(&entry, "door.practice", missing("practice", practice)));
            }
        }
        if let Some(earn) = &block.earn {
            if !set.practices.contains(&earn.practice) {
                found.push(CrossProblem::new(&entry, "earn.practice", missing("practice", &earn.practice)));
            }
        }
        if block.rules.is_empty() && !held_by.contains(block.id.as_str()) {
            found.push(CrossProblem::new(&entry, "rules", "is empty and no routine holds this block".to_string()));
        }
    }
    found
}

fn practice_problems(set: &ReadSet) -> Vec<CrossProblem> {
    let mut found = Vec::new();
    for practice in set.practices.values() {
        let entry = format!("practices/{}.json", practice.id);
        if let Some(deck) = practice.deck.as_ref().filter(|deck| !set.decks.contains(deck)) {
            found.push(CrossProblem::new(&entry, "deck", missing("deck", deck)));
        }
        if let Some(book) = practice.book.as_ref().filter(|book| !set.books.contains(book)) {
            found.push(CrossProblem::new(&entry, "book", missing("book", book)));
        }
        // A flashcard part names its own deck, which has to be in the set as well.
        for (index, part) in practice.parts.iter().enumerate() {
            if let Some(deck) = part.as_ref().filter(|deck| !set.decks.contains(deck)) {
                found.push(CrossProblem::new(&entry, &format!("parts[{index}].deck"), missing("deck", deck)));
            }
        }
    }
    found
}

fn routine_problems(set: &ReadSet) -> Vec<CrossProblem> {
    let mut found = Vec::new();
    for routine in set.routines.values() {
        let entry = format!("routines/{}.json", routine.id);
        if !set.practices.contains(&routine.practice) {
            found.push(CrossProblem::new(&entry, "practice", missing("practice", &routine.practice)));
        }
        if let Some(block) = routine.hold_blocks.iter().find(|id| !set.blocks.contains(id)) {
            found.push(CrossProblem::new(&entry, "ifSkipped.holdBlocks", missing("block", block)));
        }
    }
    found
}

/// Every cross-item problem of `set`, in the order of the file paths.
pub fn cross_problems(set: &ReadSet) -> Vec<CrossProblem> {
    let held_by: HashSet<&str> = set.routines.values().flat_map(|routine| routine.hold_blocks.iter().map(String::as_str)).collect();
    let mut found = block_problems(set, &held_by);
    found.extend(practice_problems(set));
    found.extend(routine_problems(set));
    found.sort_by(|a, b| a.entry.cmp(&b.entry));
    found
}
