//! `leko add-block --device` drafts and saves a device block: a block that names no app and no site
//! but asks the system to lock the screen, log out or shut down while its rule is closed.

mod add_block_runs;
mod run_capture;
mod scratch_folder;
mod settings_builder;

use add_block_runs::{add, assert_core_reads, saved_block};
use leko_core::io::zip_check::{check_zip, ZipVerdict};
use leko_core::models::specs::DeviceAction;
use scratch_folder::Scratch;
use serde_json::json;
use settings_builder::write_settings;
use std::collections::HashSet;

#[test]
fn a_device_block_is_drafted_in_words() {
    let scratch = Scratch::new("device-draft");
    let file = write_settings(scratch.path(), "settings.zip", &[]);
    let run = add(&file, &["--name", "Deep Work", "--device", "lock", "--for", "90m"]);
    assert_eq!((run.code, run.err.as_str()), (0, ""), "{}", run.out);
    assert!(run.out.contains("the whole computer: the screen is locked"), "{}", run.out);
    assert!(run.out.contains("Nothing was saved"), "{}", run.out);
}

#[test]
fn a_saved_device_block_reads_back_as_a_device_block() {
    let scratch = Scratch::new("device-write");
    let file = write_settings(scratch.path(), "settings.zip", &[]);
    let run = add(&file, &["--name", "Deep Work", "--device", "lock", "--always", "--write"]);
    assert_eq!((run.code, run.err.as_str()), (0, ""), "{}", run.out);
    assert!(run.out.starts_with("Saved the block \"deep-work\""), "{}", run.out);
    assert!(run.out.contains("Warning:"), "the phone cannot read a device block: {}", run.out);
    assert_core_reads(&file);
    let block = saved_block(&file, "deep-work").expect("the block is its own file");
    assert_eq!(block["device"], json!("lock"));
    assert!(block.get("covers").is_none());
    assert_eq!(block["door"], json!({"kind": "wall"}));

    match check_zip(&std::fs::read(&file).unwrap(), &HashSet::new()) {
        ZipVerdict::Read(read) => assert_eq!(read.set.blocks.get("deep-work").unwrap().device, Some(DeviceAction::Lock)),
        ZipVerdict::Refused(refusal) => panic!("{}", refusal.sentence),
    }
}

#[test]
fn a_device_block_takes_no_app_site_or_way_in() {
    let scratch = Scratch::new("device-refused");
    let file = write_settings(scratch.path(), "settings.zip", &[]);
    for flags in [
        vec!["--name", "X", "--device", "lock", "--app", "com.example.app", "--always"],
        vec!["--name", "X", "--device", "lock", "--site", "example.com", "--always"],
        vec!["--name", "X", "--device", "lock", "--wait", "30", "--always"],
        vec!["--name", "X", "--device", "lock", "--wall", "--always"],
    ] {
        let run = add(&file, &flags);
        assert_eq!(run.code, 2, "{flags:?}: {}", run.err);
        assert!(run.err.contains("device block"), "{flags:?}: {}", run.err);
    }
}

#[test]
fn a_device_word_that_names_no_action_is_not_understood() {
    let scratch = Scratch::new("device-bad-word");
    let file = write_settings(scratch.path(), "settings.zip", &[]);
    let run = add(&file, &["--name", "X", "--device", "nap", "--always"]);
    assert_eq!(run.code, 2);
    assert!(run.err.contains("lock, logoff or shutdown"), "{}", run.err);
}

#[test]
fn a_device_block_still_needs_a_name_and_a_rule() {
    let scratch = Scratch::new("device-needs");
    let file = write_settings(scratch.path(), "settings.zip", &[]);
    let no_name = add(&file, &["--device", "lock", "--always"]);
    assert_eq!(no_name.code, 2);
    assert!(no_name.err.contains("--name"), "{}", no_name.err);
    let no_rule = add(&file, &["--name", "X", "--device", "lock"]);
    assert_eq!(no_rule.code, 2);
    assert!(no_rule.err.contains("--always"), "{}", no_rule.err);
}
