mod edit_runs;
mod run_capture;
mod scratch_folder;
mod settings_builder;
use edit_runs::{command, NOW};
use leko_core::delayed_changes::waiting_file::read_waiting;
use scratch_folder::Scratch;
use serde_json::json;

fn schedule_fixture(lock_hours: Option<i64>) -> (Scratch, std::path::PathBuf) {
    let folder = Scratch::new("edit-schedule-settle");
    let mut block = settings_builder::block(
        "one",
        json!({"apps": ["com.one"]}),
        json!([settings_builder::outside_hours("h", &["mon", "tue"], &[("09:00", "17:00")])])
    );
    if let Some(hours) = lock_hours {
        block["lock"] = json!({"level": "wait", "hours": hours});
    }
    let file = settings_builder::write_settings(folder.path(), "settings.zip", &[block]);
    (folder, file)
}

#[test]
fn settle_rechecks_schedule_preconditions_and_current_locks() {
    let (_folder, file) = schedule_fixture(Some(24));
    let waiting_file = file.parent().unwrap().join("settings.zip.waiting.json");

    // Park a loosening schedule change (08:00-18:00) with a 24h wait
    let park = command(&file, "edit-schedule", &["--id", "one", "--rule", "h", "--open-hours", "08:00-18:00", "--write"], NOW);
    assert_eq!(park.code, 0, "{}", park.err);
    assert!(waiting_file.exists());

    // 1. Settle before due time (NOW + 1 hour): not due yet, writes nothing
    let early = command(&file, "settle", &["--write"], NOW + 3_600_000);
    assert_eq!(early.code, 0, "{}", early.err);
    assert!(early.out.contains("may land from"), "{}", early.out);
    assert_eq!(read_waiting(&std::fs::read_to_string(&waiting_file).unwrap()).unwrap().len(), 1);

    // 2. Settle when due (NOW + 25 hours): applies change, clears waiting record
    let due = command(&file, "settle", &["--write"], NOW + 25 * 3_600_000);
    assert_eq!(due.code, 0, "{}", due.err);
    assert!(due.out.contains("was saved"), "{}", due.out);
    assert_eq!(read_waiting(&std::fs::read_to_string(&waiting_file).unwrap()).unwrap().len(), 0);

    let zip_after = leko_core::open_zip(&std::fs::read(&file).unwrap()).unwrap();
    let block_entry: serde_json::Value = serde_json::from_slice(&zip_after.files.iter().find(|(n, _)| n == "blocks/one.json").unwrap().1).unwrap();
    assert_eq!(block_entry["rules"][0]["allowed"], json!([{"from": "08:00", "to": "18:00"}]));

    // 3. Stale precondition / conflicting change:
    // Park another change, then manually modify the block in the zip
    let park2 = command(&file, "edit-schedule", &["--id", "one", "--rule", "h", "--open-hours", "07:00-19:00", "--write"], NOW);
    assert_eq!(park2.code, 0, "{}", park2.err);

    // Now edit the block rule to something else
    let mut files = leko_core::open_zip(&std::fs::read(&file).unwrap()).unwrap().files;
    let entry = files.iter_mut().find(|(n, _)| n == "blocks/one.json").unwrap();
    let mut doc: serde_json::Value = serde_json::from_slice(&entry.1).unwrap();
    doc["rules"][0]["allowed"] = json!([{"from": "11:00", "to": "15:00"}]);
    entry.1 = serde_json::to_string_pretty(&doc).unwrap().into_bytes();
    std::fs::write(&file, settings_builder::zip_of(&files.iter().map(|(n, b)| (n.as_str(), std::str::from_utf8(b).unwrap())).collect::<Vec<_>>())).unwrap();

    // Settle at due time: precondition fails, change is kept as conflict without overwriting
    let conflict = command(&file, "settle", &["--write"], NOW + 25 * 3_600_000);
    assert_eq!(conflict.code, 0, "{}", conflict.err);
    assert!(conflict.out.contains("conflicts with current settings; it is kept"), "{}", conflict.out);
    assert_eq!(read_waiting(&std::fs::read_to_string(&waiting_file).unwrap()).unwrap().len(), 1);
}
