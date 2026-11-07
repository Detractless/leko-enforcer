//! Reads a deck file (cards and a return rule) and a book file in the phone's order of reads.
//! Ported from the deck and book half of Enforcer's `lib/read-small.js`.

use super::read_block::{is_item_id, read_item_id, ReadContext, ID_PROBLEM, MAX_ID};
use super::read_key_lock::read_lock;
use super::strict_fields::{Read, StrictFields, MAX_INT};
use crate::models::specs::ContentSpec;

const MAX_NAME: i64 = 60;
const MAX_CARDS: i64 = 5000;
const MAX_CARD_TEXT: i64 = 1000;
const MAX_REST_HOURS: i64 = 720;
const MAX_BOOK_TEXT: i64 = 1_000_000;
const MIN_WORDS_PER_PAGE: i64 = 40;
const MAX_WORDS_PER_PAGE: i64 = 300;
const MAX_RESET_TOKEN: i64 = 64;

/// JavaScript's `String.prototype.trim` whitespace: the Unicode spaces, the line terminators and U+FEFF.
fn is_js_space(c: char) -> bool {
    matches!(c, '\t' | '\n' | '\u{b}' | '\u{c}' | '\r' | ' ' | '\u{a0}' | '\u{1680}' | '\u{2000}'..='\u{200a}' | '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}' | '\u{feff}')
}

fn read_cards(fields: &mut StrictFields) -> Read<()> {
    let entries = fields.objects("cards", 1, MAX_CARDS, true)?.unwrap_or_default();
    let mut ids: Vec<String> = Vec::new();
    for mut entry in entries {
        let id = entry.text_len("id", 1, MAX_ID)?;
        if !is_item_id(&id) {
            return entry.fail("id", ID_PROBLEM);
        }
        entry.text_len("question", 1, MAX_CARD_TEXT)?;
        entry.text_len("answer", 1, MAX_CARD_TEXT)?;
        entry.finish()?;
        ids.push(id);
    }
    // The first id, by where it first appears, that appears again.
    for id in &ids {
        if ids.iter().filter(|other| *other == id).count() > 1 {
            return fields.fail("cards", &format!("has two cards with the id \"{id}\""));
        }
    }
    Ok(())
}

fn read_return_rule(parent: &mut StrictFields) -> Read<()> {
    let Some(mut fields) = parent.child("returnRule")? else {
        return Ok(());
    };
    let kind = fields.text("kind")?;
    if kind != "afterRest" && kind != "wholeDeck" {
        return fields.fail("kind", "must be afterRest or wholeDeck");
    }
    fields.whole_opt("restHours", if kind == "afterRest" { 1 } else { 0 }, MAX_REST_HOURS)?;
    fields.finish()
}

/// A book file.
pub fn read_book(fields: &mut StrictFields, context: &ReadContext) -> Read<ContentSpec> {
    let id = read_item_id(fields, context)?;
    fields.text_len("name", 1, MAX_NAME)?;
    let text = fields.text_len("text", 1, MAX_BOOK_TEXT)?;
    if text.chars().all(is_js_space) {
        return fields.fail("text", "must hold words");
    }
    fields.whole_opt("wordsPerPage", MIN_WORDS_PER_PAGE, MAX_WORDS_PER_PAGE)?;
    let at_end = fields.text_opt("atEnd", 1, MAX_INT)?;
    if at_end.as_deref().is_some_and(|word| word != "restart" && word != "stop") {
        return fields.fail("atEnd", "must be restart or stop");
    }
    fields.text_opt("resetToken", 1, MAX_RESET_TOKEN)?;
    let lock = read_lock(fields)?;
    fields.finish()?;
    Ok(ContentSpec { id, lock })
}

/// A deck file.
pub fn read_deck(fields: &mut StrictFields, context: &ReadContext) -> Read<ContentSpec> {
    let id = read_item_id(fields, context)?;
    fields.text_len("name", 1, MAX_NAME)?;
    read_cards(fields)?;
    read_return_rule(fields)?;
    fields.text_opt("resetToken", 1, MAX_RESET_TOKEN)?;
    let lock = read_lock(fields)?;
    fields.finish()?;
    Ok(ContentSpec { id, lock })
}
