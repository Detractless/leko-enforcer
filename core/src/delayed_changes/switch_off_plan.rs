//! What switching one block off means at one moment, decided from the settings alone. The change is
//! sent through the loosening judge (switching a block off loosens it), the locks that apply are
//! gathered (the block's own, and the lock of every routine that holds the block, as Enforcer's
//! `guards.js` does), and the answer is one of: it lands now, it waits until a moment, it cannot land
//! by itself, it needs a partner's key, the block is already off, or there is no such block. Nothing
//! here reads the clock or a file: the moment and the zone are arguments, and nothing is written.

use crate::config_diff::item_diff::{diff_item, DiffContext};
use crate::config_diff::loosening_table::LooseningTable;
use crate::config_time::due_time::{due, effective};
use crate::config_time::zone_time::ZoneTime;
use crate::domain::canonical::canonicalize;
use crate::domain::extended_schema::extended_schema;
use crate::models::specs::{Lock, ReadSet};
use crate::sessions::utc_moment::format_utc_moment;
use serde_json::Value;

const SECOND_MS: i64 = 1_000;

/// The answer to "switch this block off now".
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SwitchOffPlan {
    /// The settings hold no block with this id.
    NoSuchBlock,
    /// The block is already switched off.
    AlreadyOff,
    /// An applied lock carries a partner's key, which this program cannot show.
    NeedsPartnerKey,
    /// The change windows of the locks share no minute, or the moment is past the year 9999.
    CannotLand,
    /// Nothing holds the change back.
    LandsNow,
    /// The change may land at `due_at`; `because` says which locks hold it back.
    Waits { due_at: i64, because: Vec<String> },
}

/// The moment of a request: `now_ms` cut down to a whole second, the form the waiting file keeps.
pub fn request_moment(now_ms: i64) -> i64 {
    now_ms - now_ms.rem_euclid(SECOND_MS)
}

fn clock_text(minute: i64) -> String {
    format!("{:02}:{:02}", minute / 60, minute % 60)
}

/// A lock in words, for the sentence that says why a change waits.
pub fn lock_words(lock: &Lock) -> String {
    match lock {
        Lock::Open { .. } => "no wait".to_string(),
        Lock::Wait { hours, .. } => format!("a wait of {hours} hours"),
        Lock::ChangeHours { from, to, hours, .. } => format!("changes only between {} and {} after a wait of {hours} hours", clock_text(*from), clock_text(*to)),
        Lock::SealedUntil { until, .. } => format!("sealed until the end of {until}"),
        Lock::SealedAt { at, .. } => format!("sealed until {}", format_utc_moment(*at).unwrap_or_else(|| at.to_string())),
    }
}

/// Every lock that judges a change of the block `id`, each with whose it is: the block's own lock,
/// then the lock of each routine that holds the block. `None` when there is no such block.
pub fn applied_locks(set: &ReadSet, id: &str) -> Option<Vec<(String, Lock)>> {
    let block = set.blocks.get(id)?;
    let mut locks = vec![("the block's own lock".to_string(), block.lock.clone())];
    for (routine_id, routine) in &set.routines.items {
        if routine.hold_blocks.iter().any(|held| held == id) {
            locks.push((format!("the lock of the routine \"{routine_id}\", which holds the block"), routine.lock.clone()));
        }
    }
    Some(locks)
}

/// True when the judge says that writing `"on": false` into the block's file loosens it. Anything the
/// judge cannot read counts as a loosening, so the locks are never skipped by a failure.
fn judged_loosening(set: &ReadSet, id: &str, requested_at: i64, zone: &ZoneTime) -> bool {
    let Some((_, doc)) = set.docs.iter().find(|(path, _)| *path == format!("blocks/{id}.json")) else { return true };
    let mut off = doc.clone();
    let Some(map) = off.as_object_mut() else { return true };
    map.insert("on".to_string(), Value::Bool(false));
    let (Ok(old), Ok(new), Ok(table)) = (canonicalize(extended_schema(), "block", doc), canonicalize(extended_schema(), "block", &off), LooseningTable::new(extended_schema())) else {
        return true;
    };
    diff_item("block", &old, &new, &DiffContext { requested_g: requested_at, zone, table: &table }).iter().any(|unit| unit.loosens)
}

/// What switching the block `id` off means when asked for at `now_ms`, in `zone`.
pub fn plan_switch_off(set: &ReadSet, id: &str, now_ms: i64, zone: &ZoneTime) -> SwitchOffPlan {
    let (Some(block), Some(locks)) = (set.blocks.get(id), applied_locks(set, id)) else { return SwitchOffPlan::NoSuchBlock };
    if !block.on {
        return SwitchOffPlan::AlreadyOff;
    }
    let requested_at = request_moment(now_ms);
    if !judged_loosening(set, id, requested_at, zone) {
        return SwitchOffPlan::LandsNow;
    }
    if locks.iter().any(|(_, lock)| lock.key().is_some()) {
        return SwitchOffPlan::NeedsPartnerKey;
    }
    let plain: Vec<Lock> = locks.iter().map(|(_, lock)| lock.clone()).collect();
    let Some(due_at) = due(requested_at, &plain, zone) else { return SwitchOffPlan::CannotLand };
    if due_at <= requested_at {
        return SwitchOffPlan::LandsNow;
    }
    if due_at % SECOND_MS != 0 || format_utc_moment(due_at).is_none() {
        return SwitchOffPlan::CannotLand;
    }
    let because = locks
        .iter()
        .filter(|(_, lock)| !matches!(effective(lock, requested_at, zone), Lock::Open { .. }))
        .map(|(whose, lock)| format!("{whose} ({})", lock_words(lock)))
        .collect();
    SwitchOffPlan::Waits { due_at, because }
}
