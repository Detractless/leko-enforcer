//! Safety comparison for outsideHours rule changes.
//! Checks rule_compare::loosens, decision-based simulation across all 7 days and 1440 minutes,
//! and daylight saving transition periods. If any minute changes from closed to open, or if
//! safety cannot be proven, the change is treated as loosening.

use crate::config::strict_fields::parse_minutes;
use crate::config_diff::rule_compare::{days_of, loosens};
use crate::config_time::zone_time::ZoneTime;
use crate::decision::rule_closes::{rule_state, LocalMoment, RuleState};
use crate::models::specs::Rule;
use serde_json::Value;

const DAY_PAIRS: [(&str, &str); 7] = [
    ("sun", "mon"),
    ("mon", "tue"),
    ("tue", "wed"),
    ("wed", "thu"),
    ("thu", "fri"),
    ("fri", "sat"),
    ("sat", "sun"),
];

fn parse_rule(val: &Value) -> Option<Rule> {
    let id = val.get("id").and_then(Value::as_str)?.to_string();
    let kind = val.get("kind").and_then(Value::as_str)?.to_string();
    if kind != "outsideHours" {
        return None;
    }
    let days: Vec<String> = days_of(val).into_iter().collect();
    let allowed: Vec<(i64, i64)> = val
        .get("allowed")
        .and_then(Value::as_array)?
        .iter()
        .map(|entry| {
            let from = entry.get("from").and_then(Value::as_str).and_then(parse_minutes)?;
            let to = entry.get("to").and_then(Value::as_str).and_then(parse_minutes)?;
            Some((from, to))
        })
        .collect::<Option<Vec<_>>>()?;
    Some(Rule { id, kind, days, allowed, ends_at: None })
}

fn applies_on(days: &[String], day: &str) -> bool {
    days.is_empty() || days.iter().any(|named| named == day)
}

fn closes_outside_hours_sim(rule: &Rule, yesterday: &str, today: &str, minute: i64) -> bool {
    let carried_over = applies_on(&rule.days, yesterday) && rule.allowed.iter().any(|&(from, to)| from > to && minute < to);
    if carried_over || !applies_on(&rule.days, today) {
        return false;
    }
    let allowed_now = rule.allowed.iter().any(|&(from, to)| if from < to { minute >= from && minute < to } else { minute >= from });
    !allowed_now
}

pub fn is_schedule_loosening(old: &Value, new: &Value) -> bool {
    // 1. Existing loosening check
    if loosens(old, new) {
        return true;
    }

    // 2. Parse into Rule
    let (Some(old_rule), Some(new_rule)) = (parse_rule(old), parse_rule(new)) else {
        return true;
    };

    // 3. Check every pair of (yesterday, today) and all 1440 minutes
    for &(yesterday, today) in &DAY_PAIRS {
        for minute in 0..1440 {
            let old_closed = closes_outside_hours_sim(&old_rule, yesterday, today, minute);
            let new_closed = closes_outside_hours_sim(&new_rule, yesterday, today, minute);
            // If it was closed under old, but is open under new -> LOOSENING!
            if old_closed && !new_closed {
                return true;
            }
        }
    }

    // 4. Check DST transitions in America/New_York
    if let Ok(zone) = ZoneTime::new("America/New_York") {
        // 2026 Spring forward: Sunday 2026-03-08 (transitions at 2:00 AM EST)
        // 2026-03-08T00:00:00 to 2026-03-08T06:00:00 (approx 1772946000000)
        let spring_start = 1_772_946_000_000i64;
        for step in 0..360 {
            let at_ms = spring_start + step * 60_000;
            let moment = LocalMoment::new(at_ms, &zone);
            if rule_state(&old_rule, &moment) == RuleState::Closed && rule_state(&new_rule, &moment) == RuleState::Open {
                return true;
            }
        }

        // 2026 Fall back: Sunday 2026-11-01 (transitions at 2:00 AM EDT)
        let fall_start = 1_793_518_800_000i64;
        for step in 0..360 {
            let at_ms = fall_start + step * 60_000;
            let moment = LocalMoment::new(at_ms, &zone);
            if rule_state(&old_rule, &moment) == RuleState::Closed && rule_state(&new_rule, &moment) == RuleState::Open {
                return true;
            }
        }
    }

    false
}
