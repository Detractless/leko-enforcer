//! The helper modules every parked-change command test uses: run the library in this process, keep
//! what it wrote, own a scratch folder, and build the settings zips those commands read.
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

pub const NOTHING_SAVED: &str = " Nothing was saved.\n";
pub const LEFT_ALONE: &str = " It was left as it was.";

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
