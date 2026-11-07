use leko_core::delayed_changes::waiting_change::{ChangeKind, CoverageAction, CoverageOperation, WaitingChange};
use leko_core::delayed_changes::waiting_file::{read_waiting, write_waiting};
use serde_json::json;

#[test]
fn schedule_records_preserve_older_waiting_changes_and_fail_closed() {
    // 1. Format-1 switch-off record
    let off = WaitingChange::new(ChangeKind::SwitchOff, "one", 1_700_000_000_000, 1_700_000_001_000).unwrap();
    // 2. Format-2 coverage-edit record
    let edit_cov = WaitingChange::edit("two", 1_700_000_000_000, 1_700_000_002_000, vec![
        CoverageOperation { action: CoverageAction::RemoveApp, target: "com.test".into() }
    ]).unwrap();
    // 3. Format-3 schedule-edit record
    let before_rule = json!({"id": "h", "kind": "outsideHours", "days": ["mon"], "allowed": [{"from": "09:00", "to": "17:00"}]});
    let after_rule = json!({"id": "h", "kind": "outsideHours", "days": ["mon"], "allowed": [{"from": "10:00", "to": "16:00"}]});
    let edit_sched = WaitingChange::schedule("three", "h", before_rule, after_rule, 1_700_000_000_000, 1_700_000_003_000).unwrap();

    // Writing schedule change yields format 3
    let text = write_waiting(&[off.clone(), edit_cov.clone(), edit_sched.clone()]).unwrap();
    let val: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(val["format"], 3);

    // Reading format 3 recovers all 3 kinds without losing anything
    let recovered = read_waiting(&text).unwrap();
    assert_eq!(recovered.len(), 3);
    assert_eq!(recovered[0], off);
    assert_eq!(recovered[1], edit_cov);
    assert_eq!(recovered[2], edit_sched);

    // Older format 1 and format 2 files still read cleanly
    let f1_text = write_waiting(&[off.clone()]).unwrap();
    assert_eq!(serde_json::from_str::<serde_json::Value>(&f1_text).unwrap()["format"], 1);
    assert_eq!(read_waiting(&f1_text).unwrap(), [off.clone()]);

    let f2_text = write_waiting(&[off.clone(), edit_cov.clone()]).unwrap();
    assert_eq!(serde_json::from_str::<serde_json::Value>(&f2_text).unwrap()["format"], 2);
    assert_eq!(read_waiting(&f2_text).unwrap(), [off.clone(), edit_cov.clone()]);

    // Unknown fields in schedule record fail closed
    let mut corrupt = val.clone();
    corrupt["changes"][2]["unexpected"] = json!("bad");
    assert!(read_waiting(&corrupt.to_string()).is_err());

    // Unknown format fails closed
    corrupt = val.clone();
    corrupt["format"] = json!(4);
    assert!(read_waiting(&corrupt.to_string()).is_err());
}
