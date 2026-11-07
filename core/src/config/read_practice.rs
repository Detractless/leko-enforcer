//! Reads a practice file, single-activity or made of parts, in the phone's order of reads.
//! Ported from the practice half of Enforcer's `lib/read-small.js`.

use super::read_block::{is_item_id, read_item_id, ReadContext, ID_PROBLEM, MAX_ID};
use super::read_key_lock::read_lock;
use super::strict_fields::{Read, StrictFields};
use crate::models::specs::PracticeSpec;

const MAX_NAME: i64 = 60;
const ACTIVITIES: [&str; 6] = ["pushUps", "pistolSquats", "deepSquat", "flashcards", "walking", "readAloud"];
// What a part may be: a walk counts in the background, so it cannot be one.
const PART_ACTIVITIES: [&str; 4] = ["pushUps", "pistolSquats", "deepSquat", "flashcards"];
// The steps a walk may ask for, the seconds a deep squat may be held, and the pages of a reading set.
const MIN_STEPS: i64 = 100;
const MAX_STEPS: i64 = 20_000;
const MIN_HOLD_SECONDS: i64 = 5;
const MAX_HOLD_SECONDS: i64 = 600;
const MAX_PAGES: i64 = 50;
// The share of a page that must be heard.
const MIN_MATCH: i64 = 60;
const MAX_MATCH: i64 = 95;
const MIN_PARTS: i64 = 2;
const MAX_PARTS: i64 = 4;
const SINGLE_FIELDS: [&str; 7] = ["activity", "sets", "amountPerSet", "restSeconds", "deck", "book", "match"];

/// The amount of one set: steps for a walk, seconds held for a deep squat, pages for reading,
/// repetitions or cards for anything else.
fn read_amount(fields: &mut StrictFields, activity: &str) -> Read<i64> {
    match activity {
        "walking" => fields.whole("amountPerSet", MIN_STEPS, MAX_STEPS),
        "deepSquat" => fields.whole("amountPerSet", MIN_HOLD_SECONDS, MAX_HOLD_SECONDS),
        "readAloud" => fields.whole("amountPerSet", 1, MAX_PAGES),
        _ => fields.whole("amountPerSet", 1, 1000),
    }
}

/// An optional item id under `name` (a deck or a book).
fn read_item_name(fields: &mut StrictFields, name: &str) -> Read<Option<String>> {
    let value = fields.text_opt(name, 1, MAX_ID)?;
    match &value {
        Some(text) if !is_item_id(text) => fields.fail(name, ID_PROBLEM),
        _ => Ok(value),
    }
}

fn check_deck(fields: &StrictFields, activity: &str, deck: &Option<String>, what: &str) -> Read<()> {
    if activity == "flashcards" && deck.is_none() {
        return fields.fail("deck", &format!("is required for {what}"));
    }
    if activity != "flashcards" && deck.is_some() {
        return fields.fail("deck", "is only for flashcards");
    }
    Ok(())
}

fn check_book(fields: &StrictFields, activity: &str, book: &Option<String>, has_match: bool) -> Read<()> {
    if activity == "readAloud" && book.is_none() {
        return fields.fail("book", "is required for readAloud");
    }
    if activity != "readAloud" && book.is_some() {
        return fields.fail("book", "is only for readAloud");
    }
    if activity != "readAloud" && has_match {
        return fields.fail("match", "is only for readAloud");
    }
    Ok(())
}

/// One part of a practice of parts: an activity and its amount, done once; a flashcard part names
/// its own deck. Returns that deck.
fn read_part(mut fields: StrictFields) -> Read<Option<String>> {
    let code = fields.text("activity")?;
    if code == "walking" {
        return fields.fail("activity", "cannot be a part: a walk counts in the background");
    }
    if !PART_ACTIVITIES.contains(&code.as_str()) {
        return fields.fail("activity", "must be pushUps, pistolSquats, deepSquat or flashcards");
    }
    read_amount(&mut fields, &code)?;
    let deck = read_item_name(&mut fields, "deck")?;
    fields.finish()?;
    check_deck(&fields, &code, &deck, "a flashcard part")?;
    Ok(deck)
}

/// A practice of parts: the parts say what is done, so the fields of one activity are refused beside them.
fn read_mixed(fields: &mut StrictFields, id: String) -> Read<PracticeSpec> {
    for single in SINGLE_FIELDS {
        if fields.has(single) {
            return fields.fail(single, "is not used beside parts");
        }
    }
    let entries = fields.objects("parts", MIN_PARTS, MAX_PARTS, true)?.unwrap_or_default();
    let mut parts = Vec::new();
    for entry in entries {
        parts.push(read_part(entry)?);
    }
    fields.flag("verify")?;
    fields.flag("typing")?;
    let lock = read_lock(fields)?;
    fields.finish()?;
    let decks = parts.iter().flatten().cloned().collect();
    Ok(PracticeSpec { id, deck: None, decks, book: None, parts, lock })
}

/// A practice file; `decks` is every deck it draws from, its own or its flashcard parts'.
pub fn read_practice(fields: &mut StrictFields, context: &ReadContext) -> Read<PracticeSpec> {
    let id = read_item_id(fields, context)?;
    fields.text_len("name", 1, MAX_NAME)?;
    if fields.has("parts") {
        return read_mixed(fields, id);
    }
    let activity = fields.text("activity")?;
    if !ACTIVITIES.contains(&activity.as_str()) {
        return fields.fail("activity", "must be pushUps, pistolSquats, deepSquat, flashcards, walking or readAloud");
    }
    // A walk is steps in one set: sets and restSeconds are refused rather than ignored, even at their defaults.
    let walking = activity == "walking";
    if walking && fields.has("sets") {
        return fields.fail("sets", "is not used for walking");
    }
    if !walking {
        fields.whole_opt("sets", 1, 50)?;
    }
    read_amount(fields, &activity)?;
    if walking && fields.has("restSeconds") {
        return fields.fail("restSeconds", "is not used for walking");
    }
    if !walking {
        fields.whole_opt("restSeconds", 0, 600)?;
    }
    let deck = read_item_name(fields, "deck")?;
    let book = read_item_name(fields, "book")?;
    let matched = fields.whole_opt("match", MIN_MATCH, MAX_MATCH)?;
    fields.flag("verify")?;
    fields.flag("typing")?;
    let lock = read_lock(fields)?;
    fields.finish()?;
    check_deck(fields, &activity, &deck, "flashcards")?;
    check_book(fields, &activity, &book, matched.is_some())?;
    let decks = deck.iter().cloned().collect();
    Ok(PracticeSpec { id, deck, decks, book, parts: Vec::new(), lock })
}
