//! What switching a block on means (AC-001): it is a tightening, so it lands at once whatever locks the
//! block has; a block that is not there, or is already on (an `on` of true, or no `on` field), is said
//! so; and the core's loosening judge agrees that `"on"` going from false to true loosens nothing.

mod block_builder;
mod delay_sets;
mod zip_writer;

use block_builder::{moment, zone};
use delay_sets::{change_hours, locked_block, off_block, routine_holding, sealed_at, sealed_until, set_with, wait, with_partner_key};
use leko_core::config_diff::item_diff::{diff_item, DiffContext};
use leko_core::config_diff::loosening_table::LooseningTable;
use leko_core::domain::canonical::canonicalize;
use leko_core::domain::extended_schema::extended_schema;
use leko_core::switch_on::plan_switch_on::{plan_switch_on, SwitchOnPlan};
use serde_json::{json, Value};

/// The block `id` with the lock `lock`, switched off.
fn off_with(id: &str, lock: Value) -> Value {
    let mut file = locked_block(id, lock);
    file["on"] = json!(false);
    file
}

fn plan_for(blocks: &[Value], routines: &[Value], id: &str) -> SwitchOnPlan {
    plan_switch_on(&set_with(blocks, routines), id)
}

#[test]
fn a_switch_on_lands_at_once_whatever_locks_the_block_has() {
    // Every kind of lock of the block itself: none, open, a wait, change hours the clock is outside of,
    // seals until a far day and a far moment, and a partner's key on top of a wait.
    let locks = [
        Value::Null,
        json!({"level": "open"}),
        wait(24),
        wait(24 * 365),
        change_hours("08:00", "09:00", 24),
        sealed_until("2999-01-01"),
        sealed_at("2999-01-01T00:00:00Z"),
        with_partner_key(wait(24)),
    ];
    for lock in locks {
        assert_eq!(plan_for(&[off_with("a", lock.clone())], &[], "a"), SwitchOnPlan::LandsNow, "{lock}");
    }
    // A block held by a routine whose lock is closed or keyed lands just the same.
    for routine_lock in [wait(24 * 365), sealed_until("2999-01-01"), with_partner_key(wait(24)), change_hours("08:00", "09:00", 24)] {
        let routine = routine_holding("w", &["a"], routine_lock.clone());
        assert_eq!(plan_for(&[off_with("a", wait(24))], &[routine], "a"), SwitchOnPlan::LandsNow, "{routine_lock}");
    }

    // Not there, or already on: an `on` of true, no `on` field at all, and an on block with a lock.
    assert_eq!(plan_for(&[off_block("a")], &[], "b"), SwitchOnPlan::NoSuchBlock);
    assert_eq!(plan_for(&[off_block("a")], &[], "A"), SwitchOnPlan::NoSuchBlock, "ids are exact");
    assert_eq!(plan_for(&[off_block("ab")], &[], "a"), SwitchOnPlan::NoSuchBlock, "a prefix is not a match");
    assert_eq!(plan_for(&[locked_block("a", Value::Null)], &[], "a"), SwitchOnPlan::AlreadyOn, "no `on` field is on");
    let mut explicit = locked_block("a", wait(24));
    explicit["on"] = json!(true);
    assert_eq!(plan_for(&[explicit], &[], "a"), SwitchOnPlan::AlreadyOn);
    assert_eq!(plan_for(&[locked_block("a", with_partner_key(wait(24)))], &[], "a"), SwitchOnPlan::AlreadyOn);
    // The plan is about the one block asked for.
    let both = [off_block("a"), locked_block("b", Value::Null)];
    assert_eq!((plan_for(&both, &[], "a"), plan_for(&both, &[], "b")), (SwitchOnPlan::LandsNow, SwitchOnPlan::AlreadyOn));

    // The judge agrees: false to true is a change, and nothing in it loosens, for every lock.
    let table = LooseningTable::new(extended_schema()).expect("table");
    let zone = zone("America/New_York");
    let now = moment("2026-11-02T10:00:00-05:00");
    for lock in [Value::Null, wait(24), sealed_until("2999-01-01"), with_partner_key(wait(24))] {
        let off = canonicalize(extended_schema(), "block", &off_with("a", lock.clone())).expect("canonical");
        let on = canonicalize(extended_schema(), "block", &locked_block("a", lock.clone())).expect("canonical");
        let up = diff_item("block", &off, &on, &DiffContext { requested_g: now, zone: &zone, table: &table });
        assert!(up.iter().any(|unit| unit.path == "on") && up.iter().all(|unit| !unit.loosens), "{lock}: {up:?}");
        let down = diff_item("block", &on, &off, &DiffContext { requested_g: now, zone: &zone, table: &table });
        assert!(down.iter().any(|unit| unit.path == "on" && unit.loosens), "{lock}: the other way round it does loosen: {down:?}");
    }
}
