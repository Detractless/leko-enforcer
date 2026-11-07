//! Reads a routine file: its practice, when it is due, what happens when it is skipped, and its
//! snooze. Ported from the routine half of Enforcer's `lib/read-small.js`.

use super::read_block::{is_item_id, read_item_id, ReadContext, ID_PROBLEM, MAX_ID};
use super::read_door::read_practice_name;
use super::read_key_lock::read_lock;
use super::strict_fields::{parse_minutes, whole_value, Read, StrictFields, MAX_INT, MINUTES_PER_DAY};
use crate::models::specs::RoutineSpec;

const MAX_NAME: i64 = 60;
const MAX_TIMES: i64 = 12;
const MAX_TIMES_PER_DAY: i64 = 100;
// A snooze: 1 to 3 lengths of 1 to 60 minutes and 1 to 10 snoozes a day.
const MAX_SNOOZE_LENGTHS: usize = 3;
const MAX_SNOOZE_MINUTES: i64 = 60;
const MAX_SNOOZES_PER_DAY: i64 = 10;

fn read_times(fields: &mut StrictFields) -> Read<()> {
    let times = fields.texts("times", 1, MAX_TIMES, 1, MAX_INT, true)?.unwrap_or_default();
    for (index, text) in times.iter().enumerate() {
        if parse_minutes(text).is_none_or(|minutes| minutes == MINUTES_PER_DAY) {
            return fields.fail(&format!("times[{index}]"), "must be a time written HH:MM");
        }
    }
    fields.days("days")?;
    Ok(())
}

fn read_every(fields: &mut StrictFields) -> Read<()> {
    fields.whole("everyMinutes", 5, 1440)?;
    let from = fields.clock("from", false)?;
    let to = fields.clock("to", true)?;
    if from == to {
        return fields.fail("to", "must differ from \"from\"");
    }
    fields.days("days")?;
    Ok(())
}

fn read_due(parent: &mut StrictFields) -> Read<()> {
    let mut fields = parent.child_required("due")?;
    let has_times = fields.has("times");
    if has_times == fields.has("everyMinutes") {
        return fields.fail("", "must hold either \"times\" or \"everyMinutes\"");
    }
    if has_times {
        read_times(&mut fields)?;
    } else {
        read_every(&mut fields)?;
    }
    fields.finish()
}

fn read_snooze(fields: &mut StrictFields) -> Read<()> {
    if let Some(lengths) = fields.list("minutes", false)? {
        if lengths.is_empty() || lengths.len() > MAX_SNOOZE_LENGTHS {
            return fields.fail("minutes", &format!("must hold 1 to {MAX_SNOOZE_LENGTHS} entries"));
        }
        for (index, value) in lengths.iter().enumerate() {
            if !whole_value(value).is_some_and(|minutes| (1..=MAX_SNOOZE_MINUTES).contains(&minutes)) {
                return fields.fail(&format!("minutes[{index}]"), &format!("must be a whole number from 1 to {MAX_SNOOZE_MINUTES}"));
            }
        }
    }
    fields.whole_opt("perDay", 1, MAX_SNOOZES_PER_DAY)?;
    fields.finish()
}

/// What a skipped routine does; returns the blocks it holds shut.
fn read_if_skipped(parent: &mut StrictFields) -> Read<Vec<String>> {
    let Some(mut skipped) = parent.child("ifSkipped")? else {
        return Ok(Vec::new());
    };
    let hold_blocks = skipped.texts("holdBlocks", 0, MAX_INT, 1, MAX_ID, false)?.unwrap_or_default();
    skipped.flag("takeOver")?;
    skipped.flag("ring")?;
    if let Some(mut snooze) = skipped.child("snooze")? {
        read_snooze(&mut snooze)?;
    }
    skipped.finish()?;
    Ok(hold_blocks)
}

/// A routine file.
pub fn read_routine(fields: &mut StrictFields, context: &ReadContext) -> Read<RoutineSpec> {
    let id = read_item_id(fields, context)?;
    fields.text_len("name", 1, MAX_NAME)?;
    fields.flag("on")?;
    let practice = read_practice_name(fields, "practice")?;
    read_due(fields)?;
    let hold_blocks = read_if_skipped(fields)?;
    if hold_blocks.iter().any(|name| !is_item_id(name)) {
        return fields.fail("ifSkipped.holdBlocks", ID_PROBLEM);
    }
    fields.whole_opt("timesPerDay", 1, MAX_TIMES_PER_DAY)?;
    let lock = read_lock(fields)?;
    fields.finish()?;
    let mut distinct: Vec<String> = Vec::new();
    for name in hold_blocks {
        if !distinct.contains(&name) {
            distinct.push(name);
        }
    }
    Ok(RoutineSpec { id, practice, hold_blocks: distinct, lock })
}
