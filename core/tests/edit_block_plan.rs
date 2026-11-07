mod delay_sets;
mod zip_writer;
use leko_core::config_time::zone_time::ZoneTime;
use leko_core::delayed_changes::switch_off_plan::SwitchOffPlan;
use leko_core::delayed_changes::waiting_change::{CoverageAction as A, CoverageOperation};
use leko_core::edit_block::coverage::{apply_coverage_zip, CoverageApply};
use leko_core::edit_block::edit_plan::plan_edit;
use serde_json::json;

fn op(action: A, target: &str) -> CoverageOperation { CoverageOperation { action, target: target.into() } }
const NOW: i64 = 1_793_629_200_000;

#[test]
fn edit_uses_the_judge_and_all_applicable_locks() {
    let mut block = delay_sets::locked_block("one", delay_sets::wait(24));
    block["covers"]["apps"] = json!(["com.one", "com.two"]);
    let routine = delay_sets::routine_holding("r", &["one"], delay_sets::wait(48));
    let set = delay_sets::set_with(&[block.clone()], &[routine.clone()]);
    let zip = delay_sets::zip_with(&[block], &[routine]);
    let zone = ZoneTime::new("UTC").unwrap();
    let get = |ops: &[CoverageOperation]| match apply_coverage_zip(&zip, "one", ops).unwrap() { CoverageApply::Changed { old, new, .. } => (old, new), other => panic!("{other:?}") };
    let (old, new) = get(&[op(A::AddApp, "com.three")]);
    assert_eq!(plan_edit(&set, "one", &old, &new, NOW, &zone).unwrap(), SwitchOffPlan::LandsNow);
    let (old, new) = get(&[op(A::RemoveApp, "com.two"), op(A::AddApp, "com.three")]);
    let SwitchOffPlan::Waits { due_at, because } = plan_edit(&set, "one", &old, &new, NOW, &zone).unwrap() else { panic!("mixed edit must wait") };
    assert_eq!(due_at, NOW + 48 * 3_600_000);
    assert!(because.iter().any(|reason| reason.contains("routine")));
}
