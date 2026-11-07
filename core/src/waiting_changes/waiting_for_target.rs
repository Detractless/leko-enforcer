//! Which parked changes belong to an app or a website: the ones whose block covers it, each with the
//! step `leko settle` would give it at this moment (the same plan, so the two never disagree). A block
//! that covers the target counts whether or not it is closing it now; a change whose block is no longer
//! in the settings, or whose block does not cover the target, is left out. Apps that are never blocked
//! are not applied yet, so that list is empty here. Nothing here reads a clock or a file.

use crate::config_time::zone_time::ZoneTime;
use crate::decision::block_cover::{covers, Target};
use crate::delayed_changes::settle_plan::{step_of, SettleStep};
use crate::delayed_changes::waiting_change::{ChangeKind, WaitingChange};
use crate::models::specs::ReadSet;
use std::collections::HashSet;

/// The steps for the changes in `changes` whose block covers `target`, in the order given.
pub fn waiting_for_target(set: &ReadSet, target: Target, changes: &[WaitingChange], now_ms: i64, zone: &ZoneTime) -> Vec<SettleStep> {
    let never_blocked = HashSet::new();
    changes
        .iter()
        .filter(|change| {
            let covered = set.blocks.get(&change.block).is_some_and(|block| covers(block, target, &never_blocked));
            let affected = change.kind == ChangeKind::EditCoverage && change.operations.iter().any(|op| match target {
                Target::App(app) => op.action.is_app() && op.target == app,
                Target::Host(host) => !op.action.is_app() && crate::sites::site_match::covers_host(&op.target, host),
            });
            covered || affected
        })
        .map(|change| step_of(set, change, now_ms, zone))
        .collect()
}
