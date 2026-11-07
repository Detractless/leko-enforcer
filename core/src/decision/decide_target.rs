//! The answer for one app or one website at one moment: whether it is closed, which blocks and rules
//! close it (blocks in the order of their ids, rules as the file writes them), and which rules that
//! count use could not be judged. Only blocks that are on count. It never reads the clock: the moment
//! and the zone are arguments, so the same inputs always give the same answer.

use super::block_cover::{covers, Target};
use super::rule_closes::{rule_state, LocalMoment, RuleState};
use crate::config_time::zone_time::ZoneTime;
use crate::models::specs::{BlockSpec, ReadSet};
use std::collections::HashSet;

/// One rule of one block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleRef {
    pub block_id: String,
    pub rule_id: String,
    pub kind: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decision {
    /// True exactly when `closing` is not empty.
    pub closed: bool,
    pub closing: Vec<RuleRef>,
    /// Rules of covering blocks that count use (minutes, opens). Nothing counts use yet, so they are
    /// named here and never taken for "open".
    pub not_judged: Vec<RuleRef>,
}

fn rule_ref(block: &BlockSpec, rule_id: &str, kind: &str) -> RuleRef {
    RuleRef { block_id: block.id.clone(), rule_id: rule_id.to_string(), kind: kind.to_string() }
}

/// What the settings say about `target` at `at_ms`, on the wall clock of `zone`. `never_blocked` holds
/// the apps that no block may close.
pub fn decide(set: &ReadSet, target: Target, at_ms: i64, zone: &ZoneTime, never_blocked: &HashSet<String>) -> Decision {
    let now = LocalMoment::new(at_ms, zone);
    let mut blocks: Vec<&BlockSpec> = set.blocks.values().filter(|block| block.on && covers(block, target, never_blocked)).collect();
    blocks.sort_by(|first, second| first.id.cmp(&second.id));
    let (mut closing, mut not_judged) = (Vec::new(), Vec::new());
    for block in blocks {
        for rule in &block.rules {
            match rule_state(rule, &now) {
                RuleState::Closed => closing.push(rule_ref(block, &rule.id, &rule.kind)),
                RuleState::NotJudged => not_judged.push(rule_ref(block, &rule.id, &rule.kind)),
                RuleState::Open => {}
            }
        }
    }
    Decision { closed: !closing.is_empty(), closing, not_judged }
}
