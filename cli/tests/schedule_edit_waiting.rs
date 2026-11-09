mod edit_support;
use edit_support::*;
use edit_runs::{command, NOW};


#[test]
fn waiting_schedule_changes_are_visible_cancellable_and_nonconflicting() {
    let (_folder, file) = schedule_fixture(Some(24));
    let zip_bytes_orig = std::fs::read(&file).unwrap();

    // 1. Queue a schedule edit on block 'one'
    let park = command(&file, "edit-schedule", &["--id", "one", "--rule", "h", "--open-hours", "08:00-18:00", "--write"], NOW);
    assert_eq!(park.code, 0, "{}", park.err);

    // 2. Competing requests on block 'one' are refused
    let competing_sched = command(&file, "edit-schedule", &["--id", "one", "--rule", "h", "--open-hours", "07:00-19:00", "--write"], NOW);
    assert_eq!(competing_sched.code, 1);
    assert!(competing_sched.err.contains("already waiting for this block"));

    let competing_off = command(&file, "switch-off", &["--id", "one", "--write"], NOW);
    assert_eq!(competing_off.code, 1);
    assert!(competing_off.err.contains("change is waiting for this block"));

    let competing_edit = command(&file, "edit-block", &["--id", "one", "--add-app", "com.two", "--write"], NOW);
    assert_eq!(competing_edit.code, 1);
    assert!(competing_edit.err.contains("already waiting for this block"));

    // 3. Status shows the pending schedule change
    let status_text = command(&file, "status", &["--app", "com.one"], NOW);
    assert_eq!(status_text.code, 0, "{}", status_text.err);
    assert!(status_text.out.contains("schedule edit for block"), "{}", status_text.out);

    let status_json = command(&file, "status", &["--app", "com.one", "--json"], NOW);
    assert_eq!(status_json.code, 0, "{}", status_json.err);
    let json_val: serde_json::Value = serde_json::from_str(&status_json.out).unwrap();
    let waiting_arr = json_val["waiting"].as_array().unwrap();
    assert_eq!(waiting_arr.len(), 1);
    assert_eq!(waiting_arr[0]["kind"], "edit-schedule");
    assert_eq!(waiting_arr[0]["block"], "one");
    assert_eq!(waiting_arr[0]["rule"], "h");

    // 4. Cancel removes the waiting change without editing the zip
    let cancel_res = command(&file, "cancel", &["--id", "one", "--write"], NOW);
    assert_eq!(cancel_res.code, 0, "{}", cancel_res.err);
    assert!(cancel_res.out.contains("Cancelled"), "{}", cancel_res.out);
    assert_eq!(zip_bytes_orig, std::fs::read(&file).unwrap(), "cancellation must not alter settings zip");

    // Status no longer shows any waiting change
    let status_after = command(&file, "status", &["--app", "com.one", "--json"], NOW);
    let json_after: serde_json::Value = serde_json::from_str(&status_after.out).unwrap();
    assert!(json_after["waiting"].as_array().unwrap().is_empty());
}
