//! Which parked changes belong to an app or a website (AC-002): the ones whose block covers it, with
//! the step `leko settle` would give them (the very same plan), in file order. A block that covers the
//! target but is not closing it now still counts; a block that is gone, or that covers something else,
//! is left out.

mod block_builder;
mod delay_sets;
mod zip_writer;

use block_builder::{moment, zone};
use delay_sets::{locked_block, set_with, wait};
use leko_core::decision::block_cover::Target;
use leko_core::decision::decide_target::decide;
use leko_core::delayed_changes::settle_plan::{settle, step_of, SettleOutcome};
use leko_core::delayed_changes::waiting_change::{ChangeKind, WaitingChange};
use leko_core::waiting_changes::waiting_for_target::waiting_for_target;
use serde_json::{json, Value};
use std::collections::HashSet;

const NOW: &str = "2026-11-02T10:00:00-05:00";
const ZONE: &str = "America/New_York";
const HOUR_MS: i64 = 3_600_000;

fn parked(block: &str, asked_hours_before: i64, wait_hours: i64) -> WaitingChange {
    let requested = moment(NOW) - asked_hours_before * HOUR_MS;
    WaitingChange::new(ChangeKind::SwitchOff, block, requested, requested + wait_hours * HOUR_MS).expect("a good record")
}

/// A block for the app `com.<id>` that also covers the sites `sites`.
fn covering(id: &str, lock: Value, sites: &[&str]) -> Value {
    let mut file = locked_block(id, lock);
    if !sites.is_empty() {
        file["covers"]["sites"] = json!(sites);
    }
    file
}

#[test]
fn only_changes_of_blocks_that_cover_the_target_come_back_with_what_settle_would_say() {
    let mut saturday_only = covering("late", wait(24), &[]);
    saturday_only["rules"] = json!([{"id": "r", "kind": "always", "days": ["sat"]}]);
    let blocks = [covering("a", wait(24), &["example.com"]), covering("s", wait(48), &["example.com", "other.org"]), locked_block("c", wait(1)), saturday_only];
    let set = set_with(&blocks, &[]);
    let (zone, now) = (zone(ZONE), moment(NOW));
    let changes = [parked("c", 0, 1), parked("s", 2, 48), parked("gone", 5, 24), parked("a", 3, 24), parked("late", 1, 24)];

    let step_words = |target: Target| -> Vec<(String, SettleOutcome)> { waiting_for_target(&set, target, &changes, now, &zone).into_iter().map(|step| (step.change.block, step.outcome)).collect() };
    let not_yet = |hours_before: i64, wait_hours: i64, because: &str| SettleOutcome::NotYet { due_at: moment(NOW) - hours_before * HOUR_MS + wait_hours * HOUR_MS, because: vec![because.to_string()] };

    // A site: the two blocks that list it, in file order (the order the changes were parked in), and
    // not the block that covers only an app, nor the one that is gone.
    assert_eq!(
        step_words(Target::Host("example.com")),
        [("s".to_string(), not_yet(2, 48, "the block's own lock (a wait of 48 hours)")), ("a".to_string(), not_yet(3, 24, "the block's own lock (a wait of 24 hours)"))]
    );
    assert_eq!(step_words(Target::Host("sub.example.com")).len(), 2, "a subdomain is covered like the site");
    assert_eq!(step_words(Target::Host("other.org")), [("s".to_string(), not_yet(2, 48, "the block's own lock (a wait of 48 hours)"))]);
    assert!(step_words(Target::Host("nowhere.net")).is_empty());

    // An app: only the block that names it.
    assert_eq!(step_words(Target::App("com.a")), [("a".to_string(), not_yet(3, 24, "the block's own lock (a wait of 24 hours)"))]);
    assert!(step_words(Target::App("com.unknown")).is_empty());
    assert!(step_words(Target::App("example.com")).is_empty(), "an app is never matched against a site");

    // A block that covers the app but is not closing it at this moment still has its change shown.
    assert!(!decide(&set, Target::App("com.late"), now, &zone, &HashSet::new()).closed, "the block's rule is for Saturdays, so the app is open on this Monday");
    assert_eq!(step_words(Target::App("com.late")), [("late".to_string(), not_yet(1, 24, "the block's own lock (a wait of 24 hours)"))]);

    // A change that is due says so, and the answer is exactly the step the settle plan gives.
    let due = [parked("c", 3, 1)];
    let steps = waiting_for_target(&set, Target::App("com.c"), &due, now, &zone);
    assert_eq!(steps.len(), 1);
    assert_eq!(steps[0].outcome, SettleOutcome::Lands);
    for change in &changes {
        let settled = settle(&delay_sets::zip_with(&blocks, &[]), std::slice::from_ref(change), now, &zone).expect("settles");
        assert_eq!(settled.steps[0], step_of(&set, change, now, &zone), "{}: settle and the target view are one judgement", change.block);
    }
}
