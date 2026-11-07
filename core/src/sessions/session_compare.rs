//! Sessions judged by time. A session is a rule that closes a block until an exact UTC moment, then
//! counts as no rule at all. When two versions of a block are compared at the moment a change is
//! requested, a session that has ended by then is left out of both, so removing it needs no wait
//! and adding it changes nothing; a session still running is judged by when it ends: a later end
//! tightens, an earlier end loosens, removing it loosens and adding it tightens.

use super::utc_moment::parse_utc_moment;
use crate::config_diff::diff_parts::by_id;
use serde_json::Value;

const SESSION: &str = "session";

fn ends_at(rule: &Value) -> Option<i64> {
    parse_utc_moment(rule.get("endsAt")?.as_str()?)
}

/// True when `rule` is a session that has ended by `at_g`; ending at exactly that instant counts as ended.
pub fn session_ended(rule: &Value, at_g: i64) -> bool {
    rule.get("kind").and_then(Value::as_str) == Some(SESSION) && ends_at(rule).is_some_and(|end| end <= at_g)
}

/// The rules of `doc` keyed by id, without the sessions that have ended by `at_g`.
pub fn rules_in_force(doc: &Value, at_g: i64) -> Vec<(String, &Value)> {
    let mut rules = by_id(doc, "rules");
    rules.retain(|(_, rule)| !session_ended(rule, at_g));
    rules
}

/// True when the new version of a session rule ends earlier than the old one. A session whose end
/// cannot be read counts as ending earlier, the cautious answer.
pub fn session_loosens(old: &Value, new: &Value) -> bool {
    match (ends_at(old), ends_at(new)) {
        (Some(was), Some(now)) => now < was,
        _ => true,
    }
}
