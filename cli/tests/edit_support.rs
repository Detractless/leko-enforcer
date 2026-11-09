//! The helper modules every edit-command test uses: run the command, own a scratch folder, build
//! the settings zips the edit reads, and capture what it wrote.
#![allow(dead_code)]
#[path = "edit_runs/mod.rs"]
pub mod edit_runs;
#[path = "settings_test_support.rs"]
mod settings_test_support;
pub use settings_test_support::{run_capture, scratch_folder, settings_builder};
#[path = "delay_runs/mod.rs"]
pub mod delay_runs;
#[path = "../../core/tests/delay_sets/mod.rs"]
pub mod delay_sets;
#[path = "../../core/tests/zip_writer/mod.rs"]
pub mod zip_writer;

/// A block "one" whose outside-hours rule h covers Monday and Tuesday 09:00-17:00, with `covers`
/// as its app and site list, optionally held by a wait lock of `lock_hours` hours.
pub fn schedule_block(covers: serde_json::Value, lock_hours: Option<i64>) -> serde_json::Value {
    let mut block = settings_builder::block(
        "one",
        covers,
        serde_json::json!([settings_builder::outside_hours("h", &["mon", "tue"], &[("09:00", "17:00")])])
    );
    if let Some(hours) = lock_hours {
        block["lock"] = serde_json::json!({"level": "wait", "hours": hours});
    }
    block
}

/// The schedule edit fixture: a scratch folder and the settings.zip `leko edit-schedule` reads.
pub fn schedule_fixture(lock_hours: Option<i64>) -> (scratch_folder::Scratch, std::path::PathBuf) {
    let folder = scratch_folder::Scratch::new("schedule-edit");
    let file = settings_builder::write_settings(folder.path(), "settings.zip", &[schedule_block(serde_json::json!({"apps": ["com.one"]}), lock_hours)]);
    (folder, file)
}
