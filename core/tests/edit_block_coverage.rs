mod delay_sets;
mod zip_writer;
use leko_core::delayed_changes::waiting_change::{CoverageAction as A, CoverageOperation};
use leko_core::edit_block::coverage::{apply_coverage_zip, CoverageApply};
use serde_json::json;

fn op(action: A, target: &str) -> CoverageOperation { CoverageOperation { action, target: target.into() } }

#[test]
fn edit_changes_only_named_coverage_and_refuses_invalid_requests() {
    let block = delay_sets::locked_block("one", delay_sets::wait(24));
    let zip = delay_sets::zip_with(&[block.clone()], &[]);
    let changed = apply_coverage_zip(&zip, "one", &[op(A::AddApp, "com.second"), op(A::AddSite, "example.com")]).unwrap();
    let CoverageApply::Changed { old, new, bytes } = changed else { panic!("expected a change") };
    let mut expected = block.clone(); expected["covers"] = json!({"apps": ["com.one", "com.second"], "sites": ["example.com"]});
    assert_eq!(old, block); assert_eq!(new, expected);
    let before = leko_core::open_zip(&zip).unwrap().files;
    let after = leko_core::open_zip(&bytes).unwrap().files;
    for (name, data) in before.iter().filter(|(name, _)| name != "blocks/one.json") { assert_eq!(after.iter().find(|(n, _)| n == name).unwrap().1, *data); }
    assert!(matches!(apply_coverage_zip(&bytes, "one", &[op(A::AddApp, "com.second")]).unwrap(), CoverageApply::AlreadyApplied));
    assert!(matches!(apply_coverage_zip(&bytes, "one", &[op(A::AddApp, "com.second"), op(A::AddSite, "new.example")]).unwrap(), CoverageApply::Conflict));
    assert!(apply_coverage_zip(&zip, "one", &[op(A::RemoveApp, "com.one")]).is_err());
    assert!(apply_coverage_zip(&zip, "one", &[op(A::AddSite, "not a host")]).is_err());
    assert!(apply_coverage_zip(&zip, "one", &[op(A::AddApp, "x"), op(A::RemoveApp, "x")]).is_err());
    assert!(apply_coverage_zip(&zip, "../one", &[op(A::AddApp, "x")]).is_err());
    let mut all = block; all["covers"] = json!({"allAppsExcept": []});
    assert!(apply_coverage_zip(&delay_sets::zip_with(&[all], &[]), "one", &[op(A::AddApp, "x")]).is_err());
}
