mod edit_runs;
mod run_capture;
mod scratch_folder;
mod settings_builder;
use scratch_folder::Scratch;
use serde_json::json;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn schedule_fixture(lock_hours: Option<i64>) -> (Scratch, PathBuf) {
    let folder = Scratch::new("real-schedule-edit");
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

fn waiting(file: &Path) -> PathBuf {
    PathBuf::from(format!("{}.waiting.json", file.display()))
}

fn run(file: &Path, command: &str, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_leko"))
        .env("TZ", "UTC")
        .arg(command)
        .arg(file)
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn the_real_program_previews_saves_and_queues_schedule_edits() {
    let (_folder, file) = schedule_fixture(Some(1));
    let original = std::fs::read(&file).unwrap();

    // 1. Preview tightening writes nothing
    let draft = run(&file, "edit-schedule", &["--id", "one", "--rule", "h", "--open-hours", "10:00-16:00"]);
    assert!(draft.status.success(), "{}", String::from_utf8_lossy(&draft.stderr));
    assert!(String::from_utf8_lossy(&draft.stdout).contains("Nothing was saved"));
    assert_eq!(std::fs::read(&file).unwrap(), original);

    // 2. Save immediate tightening
    let saved = run(&file, "edit-schedule", &["--id", "one", "--rule", "h", "--open-hours", "10:00-16:00", "--write"]);
    assert!(saved.status.success(), "{}", String::from_utf8_lossy(&saved.stderr));
    assert!(String::from_utf8_lossy(&saved.stdout).contains("Saved"));

    // Check zip updated
    let zip_after = leko_core::open_zip(&std::fs::read(&file).unwrap()).unwrap();
    let block_entry: serde_json::Value = serde_json::from_slice(&zip_after.files.iter().find(|(n, _)| n == "blocks/one.json").unwrap().1).unwrap();
    assert_eq!(block_entry["rules"][0]["allowed"], json!([{"from": "10:00", "to": "16:00"}]));

    // Check unrelated zip entries are preserved
    let before_files = leko_core::open_zip(&original).unwrap().files;
    for (name, data) in before_files.iter().filter(|(n, _)| n != "blocks/one.json") {
        assert_eq!(zip_after.files.iter().find(|(n, _)| n == name).unwrap().1, *data);
    }

    let edited = std::fs::read(&file).unwrap();

    // 3. Park locked loosening
    let parked = run(&file, "edit-schedule", &["--id", "one", "--rule", "h", "--open-hours", "08:00-18:00", "--write"]);
    assert!(parked.status.success(), "{}", String::from_utf8_lossy(&parked.stderr));
    assert!(String::from_utf8_lossy(&parked.stdout).contains("Waiting"));
    assert_eq!(std::fs::read(&file).unwrap(), edited, "zip must not change when loosening is queued");

    // 4. Status shows pending schedule change
    let status = run(&file, "status", &["--app", "com.one"]);
    assert!(status.status.success(), "{}", String::from_utf8_lossy(&status.stderr));
    assert!(String::from_utf8_lossy(&status.stdout).contains("schedule edit for block"));

    // 5. Cancel takes the waiting change back
    let cancelled = run(&file, "cancel", &["--id", "one", "--write"]);
    assert!(cancelled.status.success(), "{}", String::from_utf8_lossy(&cancelled.stderr));
    let queue: serde_json::Value = serde_json::from_slice(&std::fs::read(waiting(&file)).unwrap()).unwrap();
    assert!(queue["changes"].as_array().unwrap().is_empty());
    assert_eq!(std::fs::read(&file).unwrap(), edited);
}
