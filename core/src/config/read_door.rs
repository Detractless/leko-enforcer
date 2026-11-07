//! Reads a block's door, pass and earn objects with the phone's limits and sentences.
//! Ported from Enforcer's `lib/read-door.js`.

use super::read_block::{is_item_id, ID_PROBLEM, MAX_ID};
use super::read_key_lock::read_key;
use super::strict_fields::{Read, StrictFields, MAX_INT};
use crate::models::specs::{Door, Earn, Pass};

const MAX_WAIT_SECONDS: i64 = 600;
const MAX_SENTENCE: i64 = 500;
const MAX_MINUTES: i64 = 1440;
const MAX_PER_DAY: i64 = 100_000;
// A walking door's pace in steps a minute.
const MIN_PACE: i64 = 60;
const MAX_PACE: i64 = 200;
const DEFAULT_PACE: i64 = 100;

/// Kotlin's `Char.isWhitespace`: the Unicode spaces and the ASCII separators, but not U+FEFF.
fn is_kotlin_space(c: char) -> bool {
    matches!(c, '\t' | '\n' | '\u{b}' | '\u{c}' | '\r' | '\u{1c}'..='\u{1f}' | ' ' | '\u{a0}' | '\u{1680}' | '\u{2000}'..='\u{200a}' | '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}')
}

pub fn read_practice_name(fields: &mut StrictFields, name: &str) -> Read<String> {
    let value = fields.text_len(name, 1, MAX_ID)?;
    if !is_item_id(&value) {
        return fields.fail(name, ID_PROBLEM);
    }
    Ok(value)
}

/// The door of a block; a missing door is a plain button, a wait of 0 seconds.
pub fn read_door(parent: &mut StrictFields) -> Read<Door> {
    let Some(mut fields) = parent.child("door")? else {
        return Ok(Door::Wait { seconds: 0 });
    };
    let kind = fields.text("kind")?;
    let door = match kind.as_str() {
        "wall" => Door::Wall,
        "wait" => Door::Wait { seconds: fields.whole("seconds", 0, MAX_WAIT_SECONDS)? },
        "sentence" => {
            let sentence = fields.text_len("sentence", 1, MAX_SENTENCE)?;
            if sentence.chars().all(is_kotlin_space) {
                return fields.fail("sentence", "must not be blank");
            }
            Door::Sentence
        }
        "key" => Door::Key { key: read_key(&mut fields.child_required("keyHash")?)? },
        "practice" => Door::Practice { practice: read_practice_name(&mut fields, "practice")? },
        "walking" => Door::Walking { steps_per_minute: fields.whole_opt("stepsPerMinute", MIN_PACE, MAX_PACE)?.unwrap_or(DEFAULT_PACE) },
        _ => return fields.fail("kind", "must be wall, wait, sentence, key, practice or walking"),
    };
    fields.finish()?;
    Ok(door)
}

fn read_pass_scope(fields: &mut StrictFields, length: &str) -> Read<String> {
    let scope = fields.text_opt("scope", 1, MAX_INT)?;
    if let Some(text) = &scope {
        if text != "block" && text != "app" {
            return fields.fail("scope", "must be block or app");
        }
    }
    if length != "oneOpen" {
        return Ok(scope.unwrap_or_else(|| "block".to_string()));
    }
    if scope.as_deref() == Some("block") {
        return fields.fail("scope", "must be app, or left out, for a oneOpen pass");
    }
    Ok("app".to_string())
}

/// The pass of a block, or `None` when the block has none.
pub fn read_pass(parent: &mut StrictFields) -> Read<Option<Pass>> {
    let Some(mut fields) = parent.child("pass")? else {
        return Ok(None);
    };
    let length = fields.text("length")?;
    if !["oneOpen", "minutes", "untilDayStart"].contains(&length.as_str()) {
        return fields.fail("length", "must be oneOpen, minutes or untilDayStart");
    }
    let minutes = match length.as_str() {
        "oneOpen" => fields.whole_opt("minutes", 1, MAX_MINUTES)?,
        "minutes" => Some(fields.whole("minutes", 1, MAX_MINUTES)?),
        _ => None,
    };
    let scope = read_pass_scope(&mut fields, &length)?;
    let per_day = fields.whole_or_word("perDay", 1, MAX_PER_DAY, "unlimited")?;
    let rising = fields.text_opt("risingCost", 1, MAX_INT)?;
    if let Some(text) = &rising {
        if text != "none" && text != "double" {
            return fields.fail("risingCost", "must be none or double");
        }
    }
    fields.finish()?;
    Ok(Some(Pass { length, minutes, scope, per_day, rising_cost: rising.unwrap_or_else(|| "none".to_string()) }))
}

/// The earn setting of a block, or `None` when the block has none.
pub fn read_earn(parent: &mut StrictFields) -> Read<Option<Earn>> {
    let Some(mut fields) = parent.child("earn")? else {
        return Ok(None);
    };
    let practice = read_practice_name(&mut fields, "practice")?;
    let minutes_each = fields.whole("minutesEach", 1, 120)?;
    let max_per_day = fields.whole("maxPerDay", 1, 600)?;
    if max_per_day < minutes_each {
        return fields.fail("maxPerDay", &format!("must be at least minutesEach ({minutes_each})"));
    }
    fields.finish()?;
    Ok(Some(Earn { practice, minutes_each, max_per_day }))
}
