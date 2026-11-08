//! Turns a block request and the current moment into the block file's JSON. The same request at the
//! same moment gives byte-identical JSON. The rule, door, pass, message and seal are written as the
//! readers expect them, so the file reads back as the words said; a request the readers could not
//! read (a wait that is too long, a timed block with a wait door) is refused here with a sentence.
//! The moment is an argument: nothing here reads the clock.

use super::block_id::id_from_name;
use super::block_request::{BlockRequest, DoorChoice, RuleChoice};
use crate::config::strict_fields::{utf16_len, WEEKDAYS};
use crate::sessions::timed_session::make_timed_session;
use serde_json::{json, Map, Value};

pub const DEFAULT_WAIT_SECONDS: i64 = 30;
pub const MAX_WAIT_SECONDS: i64 = 600;
pub const MAX_MESSAGE_UNITS: i64 = 300;
/// The pass a block with a wait door carries (the readers require one): ten minutes, three a day.
pub const PASS_MINUTES: i64 = 10;
pub const PASS_PER_DAY: i64 = 3;
pub const ALWAYS_RULE_ID: &str = "always";
pub const HOURS_RULE_ID: &str = "hours";
pub const SESSION_RULE_ID: &str = "session";

/// A block ready to be saved: its id, the file's JSON, and when a timed block ends.
#[derive(Debug, Clone, PartialEq)]
pub struct Draft {
    pub id: String,
    pub block: Value,
    /// The end of a timed block, written YYYY-MM-DDTHH:MM:SSZ.
    pub ends_at: Option<String>,
}

/// `rule` with its `days` added last, or untouched when no weekday is named (every day).
fn with_days(mut rule: Value, days: &[String]) -> Result<Value, String> {
    if let Some(bad) = days.iter().find(|day| !WEEKDAYS.contains(&day.as_str())) {
        return Err(format!("\"{bad}\" is not a weekday: use mon, tue, wed, thu, fri, sat or sun."));
    }
    if let Some(twice) = days.iter().find(|day| days.iter().filter(|other| other == day).count() > 1) {
        return Err(format!("The weekday {twice} is named twice."));
    }
    if !days.is_empty() {
        rule["days"] = json!(WEEKDAYS.iter().filter(|day| days.iter().any(|named| named == *day)).collect::<Vec<_>>());
    }
    Ok(rule)
}

fn message_problem(message: &str) -> Option<String> {
    let units = utf16_len(message);
    if units < 1 || units > MAX_MESSAGE_UNITS {
        return Some(format!("A message is 1 to {MAX_MESSAGE_UNITS} characters, not {units}."));
    }
    message.chars().any(char::is_control).then(|| "A message cannot hold a control character.".to_string())
}

/// The door the request gets: its JSON, and whether it lets a person in (so the block needs a pass).
fn door_of(request: &BlockRequest) -> Result<(Value, bool), String> {
    let timed = matches!(request.rule, RuleChoice::ForMinutes(_));
    match request.door {
        DoorChoice::Wait(_) if timed => Err("A timed block has no way in, so a wait cannot be set on it.".to_string()),
        DoorChoice::Wall | DoorChoice::Default if timed => Ok((json!({"kind": "wall"}), false)),
        DoorChoice::Wall => Ok((json!({"kind": "wall"}), false)),
        DoorChoice::Default => Ok((json!({"kind": "wait", "seconds": DEFAULT_WAIT_SECONDS}), true)),
        DoorChoice::Wait(seconds) if (0..=MAX_WAIT_SECONDS).contains(&seconds) => Ok((json!({"kind": "wait", "seconds": seconds}), true)),
        DoorChoice::Wait(seconds) => Err(format!("A wait is 0 to {MAX_WAIT_SECONDS} seconds, not {seconds}.")),
    }
}

/// The rule's JSON, the seal of a timed block, and when a timed block ends.
fn rule_of(request: &BlockRequest, now_ms: i64) -> Result<(Value, Option<Value>, Option<String>), String> {
    match &request.rule {
        RuleChoice::Always => Ok((with_days(json!({"id": ALWAYS_RULE_ID, "kind": "always"}), &request.days)?, None, None)),
        RuleChoice::OpenHours(stretches) => {
            if stretches.is_empty() {
                return Err("Open hours need at least one stretch, such as 09:00-17:00.".to_string());
            }
            let allowed: Vec<Value> = stretches.iter().map(|(from, to)| json!({"from": from, "to": to})).collect();
            Ok((with_days(json!({"id": HOURS_RULE_ID, "kind": "outsideHours", "allowed": allowed}), &request.days)?, None, None))
        }
        RuleChoice::ForMinutes(minutes) => {
            if !request.days.is_empty() {
                return Err("A timed block has no weekdays: it ends when its time is up.".to_string());
            }
            let session = make_timed_session(now_ms, *minutes, SESSION_RULE_ID)?;
            Ok((session.rule, Some(session.lock), Some(session.ends_at)))
        }
    }
}

/// The block `request` asks for, made at the moment `now_ms`, or the sentence saying why it cannot be made.
pub fn draft_block(request: &BlockRequest, now_ms: i64) -> Result<Draft, String> {
    let id = id_from_name(&request.name)?;
    if request.device.is_some() {
        if !request.apps.is_empty() || !request.sites.is_empty() {
            return Err("A device block covers no app or site: it acts on the whole computer.".to_string());
        }
    } else if request.apps.is_empty() && request.sites.is_empty() {
        return Err("A block needs at least one app or site to close.".to_string());
    }
    if let Some(problem) = request.message.as_deref().and_then(message_problem) {
        return Err(problem);
    }
    let (door, lets_in) = match request.device {
        Some(_) => {
            if matches!(request.door, DoorChoice::Wait(_)) {
                return Err("A device block has no way in, so a wait cannot be set on it.".to_string());
            }
            (json!({"kind": "wall"}), false)
        }
        None => door_of(request)?,
    };
    let (rule, seal, ends_at) = rule_of(request, now_ms)?;
    let mut covers = Map::new();
    if !request.apps.is_empty() {
        covers.insert("apps".to_string(), json!(request.apps));
    }
    if !request.sites.is_empty() {
        covers.insert("sites".to_string(), json!(request.sites));
    }
    let mut block = Map::new();
    block.insert("schema".to_string(), json!(2));
    block.insert("id".to_string(), json!(id));
    block.insert("name".to_string(), json!(request.name));
    block.insert("on".to_string(), json!(true));
    if let Some(message) = &request.message {
        block.insert("message".to_string(), json!(message));
    }
    match request.device {
        Some(action) => {
            block.insert("device".to_string(), json!(action.word()));
        }
        None => {
            block.insert("covers".to_string(), Value::Object(covers));
        }
    }
    block.insert("rules".to_string(), json!([rule]));
    block.insert("door".to_string(), door);
    if lets_in {
        block.insert("pass".to_string(), json!({"length": "minutes", "minutes": PASS_MINUTES, "perDay": PASS_PER_DAY}));
    }
    if let Some(seal) = seal {
        block.insert("lock".to_string(), seal);
    }
    Ok(Draft { id, block: Value::Object(block), ends_at })
}
