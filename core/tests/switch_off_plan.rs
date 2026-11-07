//! What switching a block off means: the change goes through the judge and the locks that apply
//! decide when it lands (AC-001); a routine that holds the block adds its lock, and a partner's key
//! stops the change (AC-002).

mod block_builder;
mod delay_sets;
mod zip_writer;

use block_builder::{moment, zone};
use delay_sets::{change_hours, locked_block, off_block, routine_holding, sealed_at, sealed_until, set_with, wait, with_partner_key};
use leko_core::config_diff::item_diff::{diff_item, DiffContext};
use leko_core::config_diff::loosening_table::LooseningTable;
use leko_core::delayed_changes::switch_off_plan::{plan_switch_off, request_moment, SwitchOffPlan};
use leko_core::domain::canonical::canonicalize;
use leko_core::domain::extended_schema::extended_schema;
use serde_json::{json, Value};

/// Monday 2 November 2026, 10:00 in New York (after the clocks went back on the 1st).
const NOW: &str = "2026-11-02T10:00:00-05:00";
const ZONE: &str = "America/New_York";
const HOUR: i64 = 3_600_000;

fn plan_for(blocks: &[Value], routines: &[Value], id: &str) -> SwitchOffPlan {
    plan_switch_off(&set_with(blocks, routines), id, moment(NOW), &zone(ZONE))
}

fn waits_until(plan: SwitchOffPlan) -> (i64, Vec<String>) {
    match plan {
        SwitchOffPlan::Waits { due_at, because } => (due_at, because),
        other => panic!("expected a wait, got {other:?}"),
    }
}

#[test]
fn the_judge_calls_writing_off_into_a_block_a_loosening() {
    let table = LooseningTable::new(extended_schema()).expect("table");
    let zone = zone(ZONE);
    let on = canonicalize(extended_schema(), "block", &locked_block("a", Value::Null)).expect("canonical");
    let off = canonicalize(extended_schema(), "block", &off_block("a")).expect("canonical");
    let units = diff_item("block", &on, &off, &DiffContext { requested_g: moment(NOW), zone: &zone, table: &table });
    assert!(units.iter().any(|unit| unit.path == "on" && unit.loosens), "{units:?}");
    let back = diff_item("block", &off, &on, &DiffContext { requested_g: moment(NOW), zone: &zone, table: &table });
    assert!(back.iter().all(|unit| !unit.loosens), "switching on tightens: {back:?}");
}

#[test]
fn a_switch_off_is_judged_and_each_kind_of_lock_gives_its_moment() {
    let now = moment(NOW);
    // No lock lands now; a block with no `on` field is on, not off; a block that is off says so.
    assert_eq!(plan_for(&[locked_block("a", Value::Null)], &[], "a"), SwitchOffPlan::LandsNow);
    assert_eq!(plan_for(&[locked_block("a", json!({"level": "open"}))], &[], "a"), SwitchOffPlan::LandsNow);
    assert_eq!(plan_for(&[off_block("a")], &[], "a"), SwitchOffPlan::AlreadyOff);
    assert_eq!(plan_for(&[locked_block("a", Value::Null)], &[], "nope"), SwitchOffPlan::NoSuchBlock);
    // A wait counts from the request.
    let (due, because) = waits_until(plan_for(&[locked_block("a", wait(24))], &[], "a"));
    assert_eq!(due, now + 24 * HOUR);
    assert_eq!(because, ["the block's own lock (a wait of 24 hours)"]);
    // Change hours: the wait first, then the first minute inside the window (22:00 the same evening).
    let (due, because) = waits_until(plan_for(&[locked_block("a", change_hours("22:00", "23:00", 2))], &[], "a"));
    assert_eq!(due, moment("2026-11-02T22:00:00-05:00"));
    assert_eq!(because, ["the block's own lock (changes only between 22:00 and 23:00 after a wait of 2 hours)"]);
    // A change hour that is already inside its window with no wait lands now.
    assert_eq!(plan_for(&[locked_block("a", change_hours("09:00", "12:00", 0))], &[], "a"), SwitchOffPlan::LandsNow);
    // A seal on a date ends at the first midnight after it in the zone; a moment seal at the moment.
    let (due, _) = waits_until(plan_for(&[locked_block("a", sealed_until("2026-11-04"))], &[], "a"));
    assert_eq!(due, moment("2026-11-05T00:00:00-05:00"));
    let (due, because) = waits_until(plan_for(&[locked_block("a", sealed_at("2026-11-03T12:00:00Z"))], &[], "a"));
    assert_eq!(due, moment("2026-11-03T12:00:00Z"));
    assert_eq!(because, ["the block's own lock (sealed until 2026-11-03T12:00:00Z)"]);
    // A seal that has ended acts as no lock.
    assert_eq!(plan_for(&[locked_block("a", sealed_until("2026-11-01"))], &[], "a"), SwitchOffPlan::LandsNow);
    assert_eq!(plan_for(&[locked_block("a", sealed_at("2026-11-02T14:59:59Z"))], &[], "a"), SwitchOffPlan::LandsNow);
    // Windows that share no minute never open together.
    let routine = routine_holding("r", &["a"], change_hours("20:00", "21:00", 0));
    assert_eq!(plan_for(&[locked_block("a", change_hours("08:00", "09:00", 0))], &[routine], "a"), SwitchOffPlan::CannotLand);
    // A due moment past the year 9999 cannot be written down, so it cannot be parked.
    assert_eq!(plan_for(&[locked_block("a", sealed_until("9999-12-31"))], &[], "a"), SwitchOffPlan::CannotLand);
    // The request moment is the whole second.
    assert_eq!(request_moment(now + 999), now);
    assert_eq!(request_moment(now), now);
}

#[test]
fn routines_that_hold_the_block_add_their_locks_and_a_partner_key_stops_the_change() {
    let now = moment(NOW);
    let block = locked_block("a", wait(24));
    // The latest moment wins, and both locks are named.
    let long = routine_holding("long", &["a"], wait(72));
    let short = routine_holding("short", &["a", "b"], wait(48));
    let (due, because) = waits_until(plan_for(&[block.clone(), locked_block("b", Value::Null)], &[long.clone(), short], "a"));
    assert_eq!(due, now + 72 * HOUR);
    assert_eq!(because.len(), 3, "{because:?}");
    assert!(because[0].starts_with("the block's own lock"), "{because:?}");
    assert!(because[1].starts_with("the lock of the routine \"long\", which holds the block"), "{because:?}");
    // A routine that holds another block adds nothing, and a routine that holds this one with no lock adds no reason.
    let elsewhere = routine_holding("elsewhere", &["b"], wait(500));
    let open = routine_holding("open", &["a"], Value::Null);
    let (due, because) = waits_until(plan_for(&[block.clone(), locked_block("b", Value::Null)], &[elsewhere, open], "a"));
    assert_eq!(due, now + 24 * HOUR);
    assert_eq!(because.len(), 1, "{because:?}");
    // A routine's lock holds a block that has no lock of its own.
    let (due, _) = waits_until(plan_for(&[locked_block("a", Value::Null)], &[long], "a"));
    assert_eq!(due, now + 72 * HOUR);
    // A partner's key on the block's lock, or on a routine's, stops the change; so does a key on an open lock.
    let keyed = locked_block("a", with_partner_key(wait(24)));
    assert_eq!(plan_for(&[keyed], &[], "a"), SwitchOffPlan::NeedsPartnerKey);
    assert_eq!(plan_for(&[block.clone()], &[routine_holding("k", &["a"], with_partner_key(wait(1)))], "a"), SwitchOffPlan::NeedsPartnerKey);
    assert_eq!(plan_for(&[locked_block("a", with_partner_key(json!({"level": "open"})))], &[], "a"), SwitchOffPlan::NeedsPartnerKey);
    // A key on a routine that does not hold the block is not in the way.
    let (due, _) = waits_until(plan_for(&[block, locked_block("b", Value::Null)], &[routine_holding("k", &["b"], with_partner_key(wait(1)))], "a"));
    assert_eq!(due, now + 24 * HOUR);
}
