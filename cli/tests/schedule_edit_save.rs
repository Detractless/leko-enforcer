mod edit_support;
use edit_support::*;
use edit_runs::{command, NOW};
use leko_core::delayed_changes::waiting_file::read_waiting;
use serde_json::json;


#[test]
fn schedule_saves_now_or_parks_without_touching_the_zip() {
    let (_folder, file) = schedule_fixture(Some(24));
    let waiting_file = file.parent().unwrap().join("settings.zip.waiting.json");

    // 1. Immediate tightening with --write: saves atomically to zip, no waiting file
    let run_tight = command(&file, "edit-schedule", &["--id", "one", "--rule", "h", "--open-hours", "10:00-16:00", "--write"], NOW);
    assert_eq!(run_tight.code, 0, "{}", run_tight.err);
    assert!(run_tight.out.contains("Saved."), "{}", run_tight.out);
    assert!(!waiting_file.exists(), "waiting file must not exist for immediate change");

    // Check that zip now holds updated rule
    let zip_after = leko_core::open_zip(&std::fs::read(&file).unwrap()).unwrap();
    let block_entry: serde_json::Value = serde_json::from_slice(&zip_after.files.iter().find(|(n, _)| n == "blocks/one.json").unwrap().1).unwrap();
    assert_eq!(block_entry["rules"][0]["allowed"], json!([{"from": "10:00", "to": "16:00"}]));

    // 2. Unchanged schedule is a no-op: does not rewrite zip or queue
    let zip_bytes_before_noop = std::fs::read(&file).unwrap();
    let run_noop = command(&file, "edit-schedule", &["--id", "one", "--rule", "h", "--open-hours", "10:00-16:00", "--write"], NOW);
    assert_eq!(run_noop.code, 0, "{}", run_noop.err);
    assert!(run_noop.out.contains("already as requested. Nothing was saved."), "{}", run_noop.out);
    assert_eq!(zip_bytes_before_noop, std::fs::read(&file).unwrap(), "no-op must not touch zip");
    assert!(!waiting_file.exists());

    // 3. Delayed change with --write: leaves zip byte-for-byte unchanged, creates parked waiting record
    let zip_bytes_before_wait = std::fs::read(&file).unwrap();
    let run_wait = command(&file, "edit-schedule", &["--id", "one", "--rule", "h", "--open-hours", "08:00-18:00", "--write"], NOW);
    assert_eq!(run_wait.code, 0, "{}", run_wait.err);
    assert!(run_wait.out.contains("Waiting; use leko settle when due."), "{}", run_wait.out);
    assert_eq!(zip_bytes_before_wait, std::fs::read(&file).unwrap(), "zip must be byte-for-byte identical when change is parked");

    assert!(waiting_file.exists(), "waiting file must be created");
    let waiting_text = std::fs::read_to_string(&waiting_file).unwrap();
    let changes = read_waiting(&waiting_text).unwrap();
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].kind, leko_core::delayed_changes::waiting_change::ChangeKind::EditSchedule);
    assert_eq!(changes[0].block, "one");
    assert_eq!(changes[0].rule.as_deref(), Some("h"));
    assert_eq!(changes[0].requested_at, NOW);
    assert_eq!(changes[0].due_at, NOW + 24 * 3_600_000);
    assert!(changes[0].before.is_some());
    assert!(changes[0].after.is_some());
}
