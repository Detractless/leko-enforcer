//! The next moment the answer for one app or one website flips between closed and open: the first
//! instant after the given one at which "some rule of a covering on block is closed" is not what it
//! is now. Rules of blocks that do not cover the target, and a rule that opens while another still
//! closes, are not flips. It scans like `next_change` (every whole minute for eight days and the end
//! of every session) and never reads the clock.

use crate::config_time::zone_time::ZoneTime;
use crate::decision::block_cover::{covers, Target};
use crate::decision::next_change::first_difference;
use crate::decision::rule_closes::{rule_state, RuleState};
use crate::models::specs::{ReadSet, Rule};
use std::collections::HashSet;

/// The first instant after `at_ms`, within eight days, at which `target` goes from closed to open or
/// from open to closed; `None` when it does not flip in that time (also for an app that is never
/// blocked), or when the look-ahead would pass the largest moment that can be counted.
pub fn next_flip(set: &ReadSet, target: Target, never_blocked: &HashSet<String>, at_ms: i64, zone: &ZoneTime) -> Option<i64> {
    let rules: Vec<&Rule> = set.blocks.values().filter(|block| block.on && covers(block, target, never_blocked)).flat_map(|block| block.rules.iter()).collect();
    first_difference(&rules, at_ms, zone, |now| rules.iter().any(|rule| rule_state(rule, now) == RuleState::Closed))
}
