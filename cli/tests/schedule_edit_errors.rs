mod edit_support;
use edit_support::*;
use delay_runs::{FailAt, Step};
use edit_runs::{command, NOW};
use leko_cli::delayed_changes::run_settle::run_settle_with;
use scratch_folder::Scratch;
use serde_json::json;

fn schedule_fixture(lock_hours: Option<i64>, with_sites: bool) -> (Scratch, std::path::PathBuf) {
    let folder = Scratch::new("edit-schedule-errors");
    let covers = if with_sites {
        json!({"apps": ["com.one"], "sites": ["example.com"]})
    } else {
        json!({"apps": ["com.one"]})
    };
    let block = schedule_block(covers, lock_hours);
    let file = settings_builder::write_settings(folder.path(), "settings.zip", &[block]);
    (folder, file)
}

#[test]
fn schedule_errors_phone_warning_and_half_saved_recovery_are_safe() {
    let (_folder, file) = schedule_fixture(Some(24), false);
    let original = std::fs::read(&file).unwrap();
    let waiting_file = file.parent().unwrap().join("settings.zip.waiting.json");

    // 1. Missing or bad arguments: exits 2 with usage
    let bad_arg = command(&file, "edit-schedule", &["--id", "one"], NOW);
    assert_eq!(bad_arg.code, 2);
    assert!(bad_arg.err.contains("Usage:"));

    // 2. Corrupt waiting file: reported with exit 1 and never overwritten
    std::fs::write(&waiting_file, b"corrupt waiting content").unwrap();
    let bad_wait = command(&file, "edit-schedule", &["--id", "one", "--rule", "h", "--open-hours", "10:00-16:00", "--write"], NOW);
    assert_eq!(bad_wait.code, 1);
    assert_eq!(std::fs::read(&waiting_file).unwrap(), b"corrupt waiting content");
    assert_eq!(std::fs::read(&file).unwrap(), original);
    std::fs::remove_file(&waiting_file).unwrap();

    // 3. Phone compatibility warning
    // Plain phone-compatible block: no phone warning
    let plain_preview = command(&file, "edit-schedule", &["--id", "one", "--rule", "h", "--open-hours", "10:00-16:00"], NOW);
    assert_eq!(plain_preview.code, 0);
    assert!(!plain_preview.out.contains("Phone Enforcer"));

    // Block with extended fields (sites): phone warning appears
    let (_folder_sites, file_sites) = schedule_fixture(Some(24), true);
    let sites_preview = command(&file_sites, "edit-schedule", &["--id", "one", "--rule", "h", "--open-hours", "10:00-16:00"], NOW);
    assert_eq!(sites_preview.code, 0);
    assert!(sites_preview.out.contains("Phone Enforcer 0.3 cannot read"), "{}", sites_preview.out);

    // 4. Halfway saved recovery on settle:
    // Park a schedule change
    assert_eq!(command(&file, "edit-schedule", &["--id", "one", "--rule", "h", "--open-hours", "08:00-18:00", "--write"], NOW).code, 0);
    let saved_queue = std::fs::read(&waiting_file).unwrap();

    // Settle with a failure during queue rewrite (Step::Rename, step 2)
    let mut failing = FailAt::new(Step::Rename, 2);
    let args = vec![file.to_string_lossy().to_string(), "--write".into()];
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = run_settle_with(&args, NOW + 24 * 3_600_000, Some("UTC"), &mut failing, &mut out, &mut err);
    assert_eq!(code, 1);
    let err_str = String::from_utf8(err).unwrap();
    assert!(err_str.contains("already saved"));

    // The zip now has the updated rule, and waiting file was preserved
    let zip_after = leko_core::open_zip(&std::fs::read(&file).unwrap()).unwrap();
    let block_entry: serde_json::Value = serde_json::from_slice(&zip_after.files.iter().find(|(n, _)| n == "blocks/one.json").unwrap().1).unwrap();
    assert_eq!(block_entry["rules"][0]["allowed"], json!([{"from": "08:00", "to": "18:00"}]));
    assert_eq!(std::fs::read(&waiting_file).unwrap(), saved_queue);

    // Rerunning settle --write clears the already-applied record safely
    let retry = command(&file, "settle", &["--write"], NOW + 24 * 3_600_000);
    assert_eq!(retry.code, 0, "{}", retry.err);
    assert!(retry.out.contains("already applied"));
    let queue_after: serde_json::Value = serde_json::from_slice(&std::fs::read(&waiting_file).unwrap()).unwrap();
    assert!(queue_after["changes"].as_array().unwrap().is_empty());
}
