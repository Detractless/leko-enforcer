mod delay_sets;
mod zip_writer;
use leko_core::edit_schedule::rule_edit::{apply_schedule_zip, ScheduleApply};
use serde_json::json;

#[test]
fn schedule_edit_changes_only_the_named_outside_hours_rule() {
    let mut block = delay_sets::locked_block("one", delay_sets::wait(24));
    block["rules"] = json!([
        {
            "id": "h",
            "kind": "outsideHours",
            "days": ["mon", "tue"],
            "allowed": [{"from": "09:00", "to": "17:00"}]
        },
        {
            "id": "other_rule",
            "kind": "always"
        }
    ]);
    let zip = delay_sets::zip_with(&[block.clone()], &[]);

    // 1. Change only allowed stretches
    let new_stretches = vec![(600, 1080)]; // 10:00 to 18:00
    let changed = apply_schedule_zip(&zip, "one", "h", None, Some(&new_stretches)).unwrap();
    let ScheduleApply::Changed { old, new, bytes } = changed else { panic!("expected a change") };

    assert_eq!(old, block);
    assert_eq!(new["rules"][0]["allowed"], json!([{"from": "10:00", "to": "18:00"}]));
    assert_eq!(new["rules"][0]["days"], json!(["mon", "tue"]));
    assert_eq!(new["rules"][1], block["rules"][1]); // other rule unchanged
    assert_eq!(new["lock"], block["lock"]); // lock unchanged
    assert_eq!(new["covers"], block["covers"]); // covers unchanged

    // Check unrelated zip entries are byte-for-byte identical
    let before_files = leko_core::open_zip(&zip).unwrap().files;
    let after_files = leko_core::open_zip(&bytes).unwrap().files;
    for (name, data) in before_files.iter().filter(|(name, _)| name != "blocks/one.json") {
        assert_eq!(after_files.iter().find(|(n, _)| n == name).unwrap().1, *data);
    }

    // 2. Change weekdays only
    let new_days = vec!["wed".to_string(), "thu".to_string(), "fri".to_string()];
    let changed_days = apply_schedule_zip(&zip, "one", "h", Some(&new_days), None).unwrap();
    let ScheduleApply::Changed { new: new_days_doc, .. } = changed_days else { panic!("expected days change") };
    assert_eq!(new_days_doc["rules"][0]["days"], json!(["wed", "thu", "fri"]));
    assert_eq!(new_days_doc["rules"][0]["allowed"], block["rules"][0]["allowed"]);

    // 3. No-op returns AlreadyApplied
    let existing_days = vec!["mon".to_string(), "tue".to_string()];
    let existing_stretches = vec![(540, 1020)];
    let noop = apply_schedule_zip(&zip, "one", "h", Some(&existing_days), Some(&existing_stretches)).unwrap();
    assert!(matches!(noop, ScheduleApply::AlreadyApplied));

    // 4. Unknown block ID
    assert!(apply_schedule_zip(&zip, "nonexistent", "h", None, Some(&new_stretches)).is_err());

    // 5. Unknown rule ID
    assert!(apply_schedule_zip(&zip, "one", "nonexistent_rule", None, Some(&new_stretches)).is_err());

    // 6. Targeting a non-outsideHours rule (e.g. other_rule which is 'always')
    assert!(apply_schedule_zip(&zip, "one", "other_rule", None, Some(&new_stretches)).is_err());

    // 7. Invalid weekdays (unknown day name or duplicates)
    assert!(apply_schedule_zip(&zip, "one", "h", Some(&["funday".to_string()]), None).is_err());
    assert!(apply_schedule_zip(&zip, "one", "h", Some(&["mon".to_string(), "mon".to_string()]), None).is_err());

    // 8. Invalid stretches (overlapping or equal from/to)
    assert!(apply_schedule_zip(&zip, "one", "h", None, Some(&[(600, 600)])).is_err());
    assert!(apply_schedule_zip(&zip, "one", "h", None, Some(&[(600, 900), (800, 1100)])).is_err());
}
