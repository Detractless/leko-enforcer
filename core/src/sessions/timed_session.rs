//! The session maker: given the current moment and a number of minutes, a session rule and a
//! moment seal that end at the same instant. The moment is an argument, never read here. A part of
//! a second rounds up to the next whole second, so a session is never shorter than asked. The rule
//! and the seal are file JSON; they read back through the readers like any hand-written ones.

use super::utc_moment::format_utc_moment;
use crate::config::read_block::{is_item_id, ID_PROBLEM};
use serde_json::{json, Value};

pub const MIN_SESSION_MINUTES: i64 = 1;
pub const MAX_SESSION_MINUTES: i64 = 10_080;
const MINUTE_MS: i64 = 60_000;
const SECOND_MS: i64 = 1_000;

/// A session rule and the seal that holds it, ending together.
#[derive(Debug, Clone, PartialEq)]
pub struct TimedSession {
    /// The rule: `{"id", "kind": "session", "endsAt"}`.
    pub rule: Value,
    /// The lock: `{"level": "sealedAt", "at"}`.
    pub lock: Value,
    /// The end, written YYYY-MM-DDTHH:MM:SSZ.
    pub ends_at: String,
    /// The same end as milliseconds since the epoch.
    pub ends_ms: i64,
}

fn end_instant(now_ms: i64, minutes: i64) -> Option<i64> {
    let end = now_ms.checked_add(minutes.checked_mul(MINUTE_MS)?)?;
    match end.rem_euclid(SECOND_MS) {
        0 => Some(end),
        part => end.checked_add(SECOND_MS - part),
    }
}

/// A session of `minutes` (1 to 10080) starting at `now_ms`, its rule named `rule_id`.
pub fn make_timed_session(now_ms: i64, minutes: i64, rule_id: &str) -> Result<TimedSession, String> {
    if !is_item_id(rule_id) {
        return Err(format!("The rule id {ID_PROBLEM}."));
    }
    if minutes < MIN_SESSION_MINUTES {
        return Err(format!("A session lasts at least {MIN_SESSION_MINUTES} minute, not {minutes}."));
    }
    if minutes > MAX_SESSION_MINUTES {
        return Err(format!("A session lasts at most {MAX_SESSION_MINUTES} minutes (7 days), not {minutes}; use a dated seal for longer."));
    }
    let too_far = || "The session would end too far in the future to write down.".to_string();
    let ends_ms = end_instant(now_ms, minutes).ok_or_else(too_far)?;
    let ends_at = format_utc_moment(ends_ms).ok_or_else(too_far)?;
    let rule = json!({"id": rule_id, "kind": "session", "endsAt": ends_at});
    let lock = json!({"level": "sealedAt", "at": ends_at});
    Ok(TimedSession { rule, lock, ends_at, ends_ms })
}
