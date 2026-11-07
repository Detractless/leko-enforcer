use leko_core::delayed_changes::waiting_change::{ChangeKind, CoverageAction, CoverageOperation, WaitingChange};
use leko_core::delayed_changes::waiting_file::{read_waiting, write_waiting};
use serde_json::json;

#[test]
fn waiting_formats_preserve_old_records_and_refuse_unknown_data() {
    let old = WaitingChange::new(ChangeKind::SwitchOff, "old", 1_700_000_000_000, 1_700_000_001_000).unwrap();
    let text = write_waiting(&[old.clone()]).unwrap();
    assert_eq!(serde_json::from_str::<serde_json::Value>(&text).unwrap()["format"], 1);
    assert_eq!(read_waiting(&text).unwrap(), [old.clone()]);
    let edit = WaitingChange::edit("new", 1_700_000_000_000, 1_700_000_002_000, vec![CoverageOperation { action: CoverageAction::RemoveApp, target: "com.test".into() }]).unwrap();
    let mixed = write_waiting(&[old.clone(), edit.clone()]).unwrap();
    let mut value: serde_json::Value = serde_json::from_str(&mixed).unwrap();
    assert_eq!(value["format"], 2);
    assert_eq!(read_waiting(&mixed).unwrap(), [old, edit.clone()]);
    value["changes"][1]["operations"][0]["surprise"] = json!(true);
    assert!(read_waiting(&value.to_string()).is_err());
    value["format"] = json!(99);
    assert!(read_waiting(&value.to_string()).is_err());
    assert!(write_waiting(&[edit.clone(), edit]).is_err());
}
