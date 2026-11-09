//! The helpers every status-command test uses: a scratch folder with a settings.zip inside, and the
//! one-whole-sentence check every status answer keeps.
#![allow(dead_code)]

#[path = "run_capture/mod.rs"]
pub mod run_capture;
#[path = "scratch_folder/mod.rs"]
pub mod scratch_folder;
#[path = "settings_builder/mod.rs"]
pub mod settings_builder;

use run_capture::Outcome;

/// A settings.zip in a folder the test owns holding `blocks`, labelled `label`.
pub fn settings(label: &str, blocks: &[serde_json::Value]) -> (scratch_folder::Scratch, std::path::PathBuf) {
    let folder = scratch_folder::Scratch::new(label);
    let file = settings_builder::write_settings(folder.path(), "settings.zip", blocks);
    (folder, file)
}

/// The one line on stdout, exit 0 and empty stderr — what every status answer must keep.
pub fn answered(run: &Outcome) -> &str {
    assert_eq!((run.code, run.err.as_str()), (0, ""), "{}", run.err);
    assert!(run.out.ends_with('\n') && run.out.matches('\n').count() == 1, "one line: {:?}", run.out);
    &run.out
}
