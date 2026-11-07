//! Reads the top-level settings file (`general.json`): day start, theme, emergency passes,
//! uninstall protection and its lock. Ported from the general half of Enforcer's `lib/read-small.js`.

use super::read_block::{is_package, ReadContext};
use super::read_key_lock::{read_key, read_lock};
use super::strict_fields::{parse_minutes, Read, StrictFields, MAX_INT, MINUTES_PER_DAY};
use crate::models::specs::{Basics, Protection};

const MAX_PACKAGE: i64 = 255;
const MAX_EMERGENCY_PER_DAY: i64 = 3;

fn read_protection(parent: &mut StrictFields) -> Read<Option<Protection>> {
    let Some(mut fields) = parent.child("uninstallProtection")? else {
        return Ok(None);
    };
    let mode = fields.text("mode")?;
    let protection = match mode.as_str() {
        "key" => Protection::Key { key: read_key(&mut fields.child_required("keyHash")?)? },
        "wait" => Protection::Wait { days: fields.whole("days", 1, 365)? },
        _ => return fields.fail("mode", "must be key or wait"),
    };
    fields.finish()?;
    Ok(Some(protection))
}

fn read_emergency(parent: &mut StrictFields) -> Read<()> {
    let Some(mut fields) = parent.child("emergency")? else {
        return Ok(());
    };
    // Counted per day: a file naming perWeek is refused.
    if fields.has("perWeek") {
        return fields.fail("perWeek", "is gone: the emergency pass is counted per day, as \"emergency.perDay\"");
    }
    fields.whole_opt("perDay", 1, MAX_EMERGENCY_PER_DAY)?;
    fields.whole_opt("minutes", 1, 60)?;
    fields.finish()
}

fn read_day_start(fields: &mut StrictFields) -> Read<()> {
    let Some(text) = fields.text_opt("dayStart", 1, MAX_INT)? else {
        return Ok(());
    };
    match parse_minutes(&text) {
        Some(minutes) if minutes % 60 == 0 && minutes < MINUTES_PER_DAY => Ok(()),
        _ => fields.fail("dayStart", "must be a whole hour written HH:00, from 00:00 to 23:00"),
    }
}

/// The apps a take-over never covers; any package may be named, as on the phone.
fn read_never_take_over(fields: &mut StrictFields) -> Read<()> {
    let packages = fields.texts("neverTakeOver", 0, MAX_INT, 1, MAX_PACKAGE, false)?.unwrap_or_default();
    for (index, text) in packages.iter().enumerate() {
        if !is_package(text) {
            return fields.fail(&format!("neverTakeOver[{index}]"), "must be a package name");
        }
    }
    Ok(())
}

/// The general file. It names no item, so the context is not used.
pub fn read_basics(fields: &mut StrictFields, _context: &ReadContext) -> Read<Basics> {
    read_day_start(fields)?;
    let theme = fields.text_opt("theme", 1, MAX_INT)?;
    if theme.as_deref().is_some_and(|name| !["system", "light", "dark"].contains(&name)) {
        return fields.fail("theme", "must be system, light or dark");
    }
    fields.flag("tracking")?;
    fields.flag("countdownBubble")?;
    read_emergency(fields)?;
    let protection = read_protection(fields)?;
    let lock = read_lock(fields)?;
    read_never_take_over(fields)?;
    // Both on when left out; the phone writes them only when false.
    fields.flag("practiceRecording")?;
    fields.flag("repSound")?;
    fields.finish()?;
    Ok(Basics { protection, lock })
}
