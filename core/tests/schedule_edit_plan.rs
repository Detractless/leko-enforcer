mod delay_sets;
mod zip_writer;
use leko_core::config_time::zone_time::ZoneTime;
use leko_core::delayed_changes::switch_off_plan::SwitchOffPlan;
use leko_core::edit_schedule::rule_edit::{apply_schedule_document, ScheduleDocument};
use leko_core::edit_schedule::schedule_plan::plan_schedule_edit;
use serde_json::json;

const NOW: i64 = 1_793_629_200_000; // A whole second UTC moment

#[test]
fn schedule_plan_applies_all_locks_to_possible_loosenings() {
    let mut block = delay_sets::locked_block("one", delay_sets::wait(24));
    block["rules"] = json!([{
        "id": "h",
        "kind": "outsideHours",
        "days": ["mon", "tue"],
        "allowed": [{"from": "09:00", "to": "17:00"}]
    }]);
    let routine = delay_sets::routine_holding("r", &["one"], delay_sets::wait(48));
    let set = delay_sets::set_with(&[block.clone()], &[routine.clone()]);
    let zone = ZoneTime::new("UTC").unwrap();

    let get_new = |days: Option<&[String]>, stretches: Option<&[(i64, i64)]>| {
        match apply_schedule_document(&block, "h", days, stretches).unwrap() {
            ScheduleDocument::Changed(new_doc) => new_doc,
            other => panic!("{other:?}"),
        }
    };

    // 1. Tightening change lands now
    let tight = get_new(None, Some(&[(600, 960)])); // 10:00 to 16:00
    assert_eq!(
        plan_schedule_edit(&set, "one", "h", &block, &tight, NOW, &zone).unwrap(),
        SwitchOffPlan::LandsNow
    );

    // 2. Loosening change (widening allowed hours) waits for the maximum lock (48h routine lock)
    let wide = get_new(None, Some(&[(480, 1080)])); // 08:00 to 18:00
    let plan = plan_schedule_edit(&set, "one", "h", &block, &wide, NOW, &zone).unwrap();
    let SwitchOffPlan::Waits { due_at, because } = plan else {
        panic!("widened schedule must wait, got {plan:?}");
    };
    assert_eq!(due_at, NOW + 48 * 3_600_000);
    assert!(because.iter().any(|r| r.contains("routine")));

    // 3. Overnight weekday addition (AC-003 / AC-004 regression case) must wait
    let mut overnight_block = delay_sets::locked_block("one", delay_sets::wait(24));
    overnight_block["rules"] = json!([{
        "id": "h",
        "kind": "outsideHours",
        "days": ["mon"],
        "allowed": [{"from": "22:00", "to": "06:00"}]
    }]);
    let overnight_set = delay_sets::set_with(&[overnight_block.clone()], &[]);
    let overnight_new = apply_schedule_document(
        &overnight_block,
        "h",
        Some(&["mon".to_string(), "sun".to_string()]),
        None
    ).unwrap();
    let ScheduleDocument::Changed(overnight_new_doc) = overnight_new else { panic!("expected change") };
    let plan = plan_schedule_edit(&overnight_set, "one", "h", &overnight_block, &overnight_new_doc, NOW, &zone).unwrap();
    assert!(matches!(plan, SwitchOffPlan::Waits { .. }), "overnight weekday addition must wait");

    // 4. Partner key lock cannot be bypassed
    let partner_lock = delay_sets::with_partner_key(delay_sets::wait(24));
    let mut keyed_block = block.clone();
    keyed_block["lock"] = partner_lock;
    let keyed_set = delay_sets::set_with(&[keyed_block.clone()], &[]);
    assert_eq!(
        plan_schedule_edit(&keyed_set, "one", "h", &keyed_block, &wide, NOW, &zone).unwrap(),
        SwitchOffPlan::NeedsPartnerKey
    );
}
