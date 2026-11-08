//! What the command says about one answer: one sentence for a person, with local times and offsets,
//! and one JSON line for a script, with UTC times. Both are built from the same values the core
//! returned, so they cannot disagree with it. Names that came from the user or from a file are written
//! with `shown`, so an odd or long one cannot flood or steer the terminal (the zone name is one the
//! core knew, so it is written as it is).

use super::status_arguments::StatusTarget;
use crate::session::session_report::shown;
use crate::waiting_changes::status_waiting::{waiting_json, waiting_lines};
use leko_core::config_time::zone_time::ZoneTime;
use leko_core::decision::decide_target::{Decision, RuleRef};
use leko_core::models::specs::DeviceAction;
use leko_core::decision::next_change::LOOK_AHEAD_DAYS;
use leko_core::delayed_changes::settle_plan::SettleStep;
use leko_core::sessions::utc_moment::format_utc_moment;
use serde_json::{json, Value};

/// Everything one answer is made of.
pub struct Answer<'a> {
    pub target: &'a StatusTarget,
    pub at_ms: i64,
    pub zone_name: &'a str,
    pub zone: &'a ZoneTime,
    pub decision: &'a Decision,
    /// The next moment the answer flips, within the look-ahead.
    pub until: Option<i64>,
    /// The parked changes of the blocks that cover the target; `None` when the waiting file could not
    /// be read (nothing is listed in the sentence, and the JSON holds `null`).
    pub waiting: Option<&'a [SettleStep]>,
}

/// Everything one device answer is made of.
pub struct DeviceAnswer<'a> {
    pub at_ms: i64,
    pub zone_name: &'a str,
    pub zone: &'a ZoneTime,
    /// The strongest action the device blocks ask for, or `None` when none is closed.
    pub action: Option<DeviceAction>,
    /// Every closed device rule, as (block, rule, kind, action).
    pub closing: &'a [(String, String, String, DeviceAction)],
    /// The next moment the action changes, within the look-ahead.
    pub until: Option<i64>,
}

fn device_rules(rules: &[(String, String, String, DeviceAction)]) -> Vec<RuleRef> {
    rules.iter().map(|(block, rule, kind, _)| RuleRef { block_id: block.clone(), rule_id: rule.clone(), kind: kind.clone() }).collect()
}

fn rules_text(rules: &[RuleRef]) -> String {
    let parts: Vec<String> = rules.iter().map(|rule| format!("block {} rule {} ({})", shown(&rule.block_id), shown(&rule.rule_id), rule.kind)).collect();
    parts.join("; ")
}

fn rules_json(rules: &[RuleRef]) -> Value {
    Value::Array(rules.iter().map(|rule| json!({"block": rule.block_id, "rule": rule.rule_id, "kind": rule.kind})).collect())
}

/// The sentence for a person; ends with a newline.
pub fn sentence(answer: &Answer) -> String {
    let (kind, name) = match answer.target {
        StatusTarget::App(name) => ("App", name),
        StatusTarget::Site(host) => ("Site", host),
        StatusTarget::Device => unreachable!("a device answer has its own sentence"),
    };
    let state = if answer.decision.closed { "closed" } else { "open" };
    let span = match answer.until {
        Some(until) => format!("until {}", answer.zone.local_text(until)),
        None => format!("for at least the next {LOOK_AHEAD_DAYS} days"),
    };
    let mut text = format!("{kind} {} is {state} {span}, judged at {} in {}.", shown(name), answer.zone.local_text(answer.at_ms), answer.zone_name);
    if answer.decision.closed {
        text.push_str(&format!(" Closed by {}.", rules_text(&answer.decision.closing)));
    }
    if !answer.decision.not_judged.is_empty() {
        text.push_str(&format!(" Not judged, because they count use and no use is counted yet: {}.", rules_text(&answer.decision.not_judged)));
    }
    if matches!(answer.target, StatusTarget::App(_)) {
        text.push_str(" Apps that are never blocked are not applied yet.");
    }
    // With nothing waiting the sentence is the one it always was; each parked change adds a line.
    text.push_str(&waiting_lines(answer.waiting.unwrap_or_default(), answer.zone_name, answer.zone));
    text.push_str(if answer.waiting.is_some_and(|steps| !steps.is_empty()) { "\nNothing was saved or changed.\n" } else { " Nothing was saved or changed.\n" });
    text
}

/// The sentence about the whole computer; ends with a newline.
pub fn device_sentence(answer: &DeviceAnswer) -> String {
    let at = answer.zone.local_text(answer.at_ms);
    let mut text = match answer.action {
        None => format!("Device: no device block is closed, judged at {at} in {}. For at least the next {LOOK_AHEAD_DAYS} days nothing will lock, log out or shut down.", answer.zone_name),
        Some(action) => {
            let what = match action {
                DeviceAction::Lock => "the screen is locked",
                DeviceAction::Logoff => "the user is logged out",
                DeviceAction::Shutdown => "the computer is shut down",
            };
            let span = match answer.until {
                Some(until) => format!("until {}", answer.zone.local_text(until)),
                None => format!("for at least the next {LOOK_AHEAD_DAYS} days"),
            };
            format!("Device: {what} {span}, judged at {at} in {}. Closed by {}.", answer.zone_name, rules_text(&device_rules(answer.closing)))
        }
    };
    text.push_str(" Nothing was saved or changed.\n");
    text
}

/// The one line of JSON for a device answer, with UTC times (`null` when a moment has no UTC text);
/// ends with a newline.
pub fn device_json(answer: &DeviceAnswer) -> String {
    let closing: Vec<Value> = answer
        .closing
        .iter()
        .map(|(block, rule, kind, action)| json!({"block": block, "rule": rule, "kind": kind, "device": action.word()}))
        .collect();
    let line = json!({
        "target": {"kind": "device", "name": ""},
        "at": format_utc_moment(answer.at_ms),
        "zone": answer.zone_name,
        "closed": answer.action.is_some(),
        "action": answer.action.map(DeviceAction::word),
        "closing": closing,
        "until": answer.until.and_then(format_utc_moment),
    });
    format!("{line}\n")
}

/// The one line of JSON for a script, with UTC times (`null` when a moment has no UTC text); ends
/// with a newline.
pub fn json_line(answer: &Answer) -> String {
    let (kind, name) = match answer.target {
        StatusTarget::App(name) => ("app", name),
        StatusTarget::Site(host) => ("site", host),
        StatusTarget::Device => unreachable!("a device answer has its own JSON line"),
    };
    let line = json!({
        "target": {"kind": kind, "name": name},
        "at": format_utc_moment(answer.at_ms),
        "zone": answer.zone_name,
        "closed": answer.decision.closed,
        "closing": rules_json(&answer.decision.closing),
        "not_judged": rules_json(&answer.decision.not_judged),
        "until": answer.until.and_then(format_utc_moment),
        "waiting": waiting_json(answer.waiting),
    });
    format!("{line}\n")
}
