mod edit_runs;
mod run_capture;
mod scratch_folder;
mod settings_builder;
mod delay_runs;
#[path = "../../core/tests/delay_sets/mod.rs"] mod delay_sets;
#[path = "../../core/tests/zip_writer/mod.rs"] mod zip_writer;
use edit_runs::{block, command, fixture, waiting, NOW};
use serde_json::json;

#[test]
fn settle_rechecks_current_coverage_and_locks_before_applying() {
    let (folder, file) = fixture(Some(24));
    let parked = command(&file, "edit-block", &["--id", "one", "--remove-app", "com.two", "--write"], NOW);
    assert_eq!(parked.code, 0, "{}", parked.err);
    let before = std::fs::read(&file).unwrap();
    let early = command(&file, "settle", &["--write"], NOW + 24 * 3_600_000 - 1000);
    assert_eq!(early.code, 0, "{}", early.err);
    assert_eq!(before, std::fs::read(&file).unwrap());
    let mut changed = block(&file);
    changed["lock"]["hours"] = json!(48);
    changed["message"] = json!("later change");
    settings_builder::write_settings(folder.path(), "settings.zip", &[changed.clone()]);
    let still_early = command(&file, "settle", &["--write"], NOW + 24 * 3_600_000);
    assert_eq!(still_early.code, 0, "{}", still_early.err);
    assert_eq!(block(&file), changed);
    let due = command(&file, "settle", &["--write"], NOW + 48 * 3_600_000);
    assert_eq!(due.code, 0, "{}", due.err);
    let mut expected = changed; expected["covers"]["apps"] = json!(["com.one"]);
    assert_eq!(block(&file), expected);
    let queue: serde_json::Value = serde_json::from_slice(&std::fs::read(waiting(&file)).unwrap()).unwrap();
    assert!(queue["changes"].as_array().unwrap().is_empty());
}

#[test]
fn conflict_and_new_partner_key_keep_the_waiting_edit() {
    let (folder, file) = fixture(Some(24));
    assert_eq!(command(&file, "edit-block", &["--id", "one", "--remove-app", "com.two", "--add-app", "com.three", "--write"], NOW).code, 0);
    let queue = std::fs::read(waiting(&file)).unwrap();
    let mut current = block(&file);
    current["covers"]["apps"] = json!(["com.one", "com.two", "com.three"]);
    settings_builder::write_settings(folder.path(), "settings.zip", &[current.clone()]);
    let conflict = command(&file, "settle", &["--write"], NOW + 48 * 3_600_000);
    assert_eq!(conflict.code, 0, "{}", conflict.err);
    assert!(conflict.out.contains("conflicts"));
    assert_eq!(block(&file), current);
    assert_eq!(std::fs::read(waiting(&file)).unwrap(), queue);
    current["covers"]["apps"] = json!(["com.one", "com.two"]);
    current["lock"] = delay_sets::with_partner_key(delay_sets::wait(24));
    settings_builder::write_settings(folder.path(), "settings.zip", &[current.clone()]);
    let key = command(&file, "settle", &["--write"], NOW + 48 * 3_600_000);
    assert_eq!(key.code, 0, "{}", key.err);
    assert!(key.out.contains("partner key"));
    assert_eq!(block(&file), current);
    assert_eq!(std::fs::read(waiting(&file)).unwrap(), queue);
}

#[test]
fn a_saved_zip_and_failed_queue_cleanup_recover_on_retry() {
    let (_folder, file) = fixture(Some(24));
    assert_eq!(command(&file, "edit-block", &["--id", "one", "--remove-app", "com.two", "--write"], NOW).code, 0);
    let queue = std::fs::read(waiting(&file)).unwrap();
    let mut failing = delay_runs::FailAt::new(delay_runs::Step::Rename, 2);
    let args = vec![file.to_string_lossy().to_string(), "--write".into()];
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = leko_cli::delayed_changes::run_settle::run_settle_with(&args, NOW + 24 * 3_600_000, Some("UTC"), &mut failing, &mut out, &mut err);
    assert_eq!(code, 1);
    assert!(String::from_utf8(err).unwrap().contains("already saved"));
    assert_eq!(block(&file)["covers"]["apps"], json!(["com.one"]));
    assert_eq!(std::fs::read(waiting(&file)).unwrap(), queue);
    let retry = command(&file, "settle", &["--write"], NOW + 24 * 3_600_000);
    assert_eq!(retry.code, 0, "{}", retry.err);
    assert!(retry.out.contains("already applied"));
    let queue: serde_json::Value = serde_json::from_slice(&std::fs::read(waiting(&file)).unwrap()).unwrap();
    assert!(queue["changes"].as_array().unwrap().is_empty());
}
