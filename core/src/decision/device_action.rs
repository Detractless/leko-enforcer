//! Which whole-computer action, if any, the settings ask for at an instant: the strongest action of
//! the device blocks that are on and closed then, and the rules that ask for it. It never reads the
//! clock: the moment and the zone are arguments, so the same inputs always give the same answer.

use super::next_change::first_difference;
use super::rule_closes::{rule_state, LocalMoment, RuleState};
use crate::config_time::zone_time::ZoneTime;
use crate::models::specs::{BlockSpec, DeviceAction, ReadSet, Rule};

/// The device blocks that are on, in the order of their ids, so every answer is stable.
fn on_device_blocks(set: &ReadSet) -> Vec<&BlockSpec> {
    let mut blocks: Vec<_> = set.blocks.values().filter(|block| block.on && block.device.is_some()).collect();
    blocks.sort_by(|first, second| first.id.cmp(&second.id));
    blocks
}

/// The strongest action of the on device blocks that are closed at `now`.
fn strongest(set: &ReadSet, now: &LocalMoment) -> Option<DeviceAction> {
    let mut strongest: Option<DeviceAction> = None;
    for block in on_device_blocks(set) {
        let closed = block.rules.iter().any(|rule| rule_state(rule, now) == RuleState::Closed);
        if !closed {
            continue;
        }
        let action = block.device.expect("a device block names its action");
        if strongest.map_or(true, |current| action.strength() > current.strength()) {
            strongest = Some(action);
        }
    }
    strongest
}

/// The action to take at `at_ms` on the wall clock of `zone`, or `None` when no device block is
/// closed then.
pub fn device_action(set: &ReadSet, at_ms: i64, zone: &ZoneTime) -> Option<DeviceAction> {
    strongest(set, &LocalMoment::new(at_ms, zone))
}

/// Every rule of an on device block that is closed at `at_ms`, as (block id, rule id, rule kind,
/// action), in the order of block ids and then rule ids.
pub fn closed_device_rules(set: &ReadSet, at_ms: i64, zone: &ZoneTime) -> Vec<(String, String, String, DeviceAction)> {
    let now = LocalMoment::new(at_ms, zone);
    let mut found = Vec::new();
    for block in on_device_blocks(set) {
        let action = block.device.expect("a device block names its action");
        let mut rules: Vec<&Rule> = block.rules.iter().filter(|rule| rule_state(rule, &now) == RuleState::Closed).collect();
        rules.sort_by(|first, second| first.id.cmp(&second.id));
        for rule in rules {
            found.push((block.id.clone(), rule.id.clone(), rule.kind.clone(), action));
        }
    }
    found
}

/// The first moment after `at_ms`, within eight days, at which the action changes: a device block
/// closing or opening, or a session among them ending. `None` when it does not change in that time.
pub fn next_device_change(set: &ReadSet, at_ms: i64, zone: &ZoneTime) -> Option<i64> {
    let rules: Vec<&Rule> = on_device_blocks(set).iter().flat_map(|block| block.rules.iter()).collect();
    first_difference(&rules, at_ms, zone, |now| strongest(set, now))
}
