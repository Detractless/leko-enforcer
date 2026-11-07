//! Whether one rule of a block closes at an instant, read on the wall clock of one time zone. Always
//! and every-open rules close on the weekdays they name; an outside-hours rule closes outside its
//! allowed stretches, as the phone judges it; a session rule closes until its end moment. Rules that
//! count use (minutes, opens) cannot be judged here, and say so. The moment is an argument, never
//! read here. Ported from Enforcer's `RuleClosures.closesByTime`.

use crate::config_time::zone_time::ZoneTime;
use crate::models::specs::Rule;

/// What a rule says at one instant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleState {
    Closed,
    Open,
    /// A rule that counts use (minutes, opens); no use is counted yet, so it is neither.
    NotJudged,
}

/// One instant seen on the wall clock of a zone: what a rule needs to know, worked out once.
#[derive(Debug, Clone, Copy)]
pub struct LocalMoment {
    pub at_ms: i64,
    minute: i64,
    today: &'static str,
    yesterday: &'static str,
}

impl LocalMoment {
    pub fn new(at_ms: i64, zone: &ZoneTime) -> LocalMoment {
        let date = zone.date_at(at_ms);
        LocalMoment { at_ms, minute: zone.minute_of_day_at(at_ms), today: date.weekday(), yesterday: zone.plus_days(date, -1).weekday() }
    }
}

fn applies_on(days: &[String], day: &str) -> bool {
    days.is_empty() || days.iter().any(|named| named == day)
}

/// An outside-hours rule. A stretch belongs to the day it starts on and its part after midnight is
/// judged with that day's rule; after the stretch ends, a rule whose days leave out the new day no
/// longer applies. An empty allowed list closes on every day the rule applies.
fn closes_outside_hours(rule: &Rule, now: &LocalMoment) -> bool {
    let carried_over = applies_on(&rule.days, now.yesterday) && rule.allowed.iter().any(|&(from, to)| from > to && now.minute < to);
    if carried_over || !applies_on(&rule.days, now.today) {
        return false;
    }
    let allowed_now = rule.allowed.iter().any(|&(from, to)| if from < to { now.minute >= from && now.minute < to } else { now.minute >= from });
    !allowed_now
}

/// What `rule` says at `now`.
pub fn rule_state(rule: &Rule, now: &LocalMoment) -> RuleState {
    let closes = match rule.kind.as_str() {
        "always" | "everyOpen" => applies_on(&rule.days, now.today),
        "outsideHours" => closes_outside_hours(rule, now),
        "session" => rule.ends_at.is_some_and(|end| now.at_ms < end),
        _ => return RuleState::NotJudged,
    };
    if closes {
        RuleState::Closed
    } else {
        RuleState::Open
    }
}
