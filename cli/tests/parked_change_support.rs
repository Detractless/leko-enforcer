//! The helper modules every parked-change command test uses: run the library in this process, keep
//! what it wrote, own a scratch folder, and build the settings zips those commands read.
#![allow(dead_code)]
#[path = "delay_runs/mod.rs"]
pub mod delay_runs;
#[path = "../../core/tests/delay_sets/mod.rs"]
pub mod delay_sets;
#[path = "run_capture/mod.rs"]
pub mod run_capture;
#[path = "scratch_folder/mod.rs"]
pub mod scratch_folder;
#[path = "../../core/tests/zip_writer/mod.rs"]
pub mod zip_writer;

use leko_core::io::zip_check::{check_zip, ZipVerdict};
use std::collections::HashSet;
use scratch_folder::Scratch;

pub const NOTHING_SAVED: &str = " Nothing was saved.\n";
pub const LEFT_ALONE: &str = " It was left as it was.";

/// Set a scratch folder read-only and report whether this process cannot create a new file there.
pub fn folder_enforces_read_only(folder: &Scratch) -> bool {
    folder.set_mode(0o555);
    if std::fs::File::create(folder.path().join("probe")).is_ok() {
        folder.set_mode(0o755);
        return false;
    }
    true
}

/// Check a save failure in a read-only folder and confirm that its contents remain unchanged.
pub fn assert_read_only_folder_blocks_save(folder: &Scratch, check_failure: impl FnOnce()) {
    let before = delay_runs::snapshot(folder.path());
    if folder_enforces_read_only(folder) {
        check_failure();
        folder.set_mode(0o755);
        assert_eq!(
            delay_runs::snapshot(folder.path()),
            before,
            "nothing came, went or changed"
        );
    }
    folder.set_mode(0o755);
}

/// The core's sentence for refusing these bytes as a settings zip.
pub fn core_refusal(bytes: &[u8]) -> String {
    match check_zip(bytes, &HashSet::new()) {
        ZipVerdict::Refused(refusal) => refusal.sentence,
        ZipVerdict::Read(_) => panic!("the core was expected to refuse these {} bytes", bytes.len()),
    }
}

/// `sentence` with the one-line "Nothing was saved." ending the file tests all expect.
pub fn nothing_saved(sentence: &str) -> String {
    let stop = if sentence.ends_with(['.', '!', '?']) { "" } else { "." };
    format!("{sentence}{stop}{NOTHING_SAVED}")
}

/// A locked block written as switched off, for the switch-on tests.
pub fn off_with(id: &str, lock: serde_json::Value) -> serde_json::Value {
    let mut file = delay_sets::locked_block(id, lock);
    file["on"] = serde_json::json!(false);
    file
}
