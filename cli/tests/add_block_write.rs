//! With `--write` the block shown in the draft is saved as `blocks/ID.json` and every other file of
//! the zip keeps its name and content (AC-002): on built settings, on zips with no blocks or only
//! switched-off ones, and on every zip of the phone's corpus that the phone reads.

mod add_block_runs;
#[path = "../../core/tests/corpus_zips/mod.rs"]
mod corpus_zips;
mod run_capture;
mod scratch_folder;
mod settings_builder;
#[path = "../../core/tests/zip_writer/mod.rs"]
mod zip_writer;

use add_block_runs::{add, assert_core_reads, entries_of, saved_block, APP, NOW};
use corpus_zips::{case_bytes, load_corpus};
use leko_core::blocks::block_request::{BlockRequest, DoorChoice, RuleChoice};
use leko_core::blocks::draft_block::draft_block;
use run_capture::moment;
use scratch_folder::Scratch;
use serde_json::json;
use settings_builder::{always, block, block_with, write_settings, zip_of};
use zip_writer::Method;

const FLAGS: [&str; 5] = ["--name", "Social Media", "--app", APP, "--always"];

fn the_request() -> BlockRequest {
    BlockRequest { name: "Social Media".into(), apps: vec![APP.into()], sites: vec![], rule: RuleChoice::Always, days: vec![], door: DoorChoice::Default, message: None }
}

/// Runs `--write` on the zip `bytes` in a fresh folder; returns the folder and the file's path.
fn write_into(label: &str, bytes: &[u8]) -> (Scratch, std::path::PathBuf) {
    let folder = Scratch::new(label);
    let file = folder.path().join("settings.zip");
    std::fs::write(&file, bytes).unwrap();
    (folder, file)
}

/// Every file of `before` is in `after` with the same bytes, and `after` holds the new block too.
fn assert_only_the_block_was_added(before: &[(String, Vec<u8>)], after: &[(String, Vec<u8>)]) {
    let new: Vec<&(String, Vec<u8>)> = after.iter().filter(|entry| !before.contains(entry)).collect();
    assert_eq!(new.len(), 1, "exactly one entry is new: {:?}", new.iter().map(|entry| &entry.0).collect::<Vec<_>>());
    assert_eq!(new[0].0, "blocks/social-media.json");
    assert_eq!(after.len(), before.len() + 1, "no other entry was lost or duplicated");
    for entry in before {
        assert!(after.contains(entry), "{} was changed or lost", entry.0);
    }
}

#[test]
fn write_saves_the_drafted_block_and_leaves_every_other_file_as_it_was() {
    // Built settings: a block that is on, one that is switched off, and the usual other files.
    let blocks = [block("old", json!({"apps": ["com.old.app"]}), json!([always("r", &[])])), block_with("off", json!({"apps": ["com.off.app"]}), json!([always("r", &[])]), json!({"on": false}))];
    let folder = Scratch::new("write");
    let file = write_settings(folder.path(), "settings.zip", &blocks);
    let before = entries_of(&file);
    let run = add(&file, &[&FLAGS[..], &["--write"]].concat());
    assert_eq!((run.code, run.err.as_str()), (0, ""), "{}", run.out);
    assert!(run.out.starts_with("Saved the block \"social-media\" as blocks/social-media.json"), "{}", run.out);
    assert_eq!(folder.names(), ["settings.zip"], "no temporary file is left");
    assert_core_reads(&file);
    let after = entries_of(&file);
    assert_only_the_block_was_added(&before, &after);
    let drafted = draft_block(&the_request(), moment(NOW)).unwrap();
    assert_eq!(saved_block(&file, "social-media"), Some(drafted.block.clone()), "the saved block is the drafted one");
    let general = |entries: &[(String, Vec<u8>)]| entries.iter().find(|entry| entry.0 == "general.json").unwrap().1.clone();
    assert_eq!(general(&after), general(&before), "general.json is not rewritten");
    let text = String::from_utf8(after.iter().find(|entry| entry.0 == "blocks/social-media.json").unwrap().1.clone()).unwrap();
    assert!(text.ends_with("}\n") && text.contains("\n  \"id\": \"social-media\""), "the block file is readable, indented JSON: {text}");

    // A zip with no blocks, and a zip with only switched-off blocks, take their first block.
    let none = zip_of(&[("GUIDE.md", "guide"), ("general.json", r#"{"schema": 2}"#)]);
    let only_off = settings_builder::settings_bytes(&blocks[1..]);
    for (label, bytes) in [("no-blocks", none), ("only-off", only_off)] {
        let (_folder, file) = write_into(label, &bytes);
        let before = entries_of(&file);
        let run = add(&file, &[&FLAGS[..], &["--write"]].concat());
        assert_eq!((run.code, run.err.as_str()), (0, ""), "{label}: {}", run.out);
        assert_core_reads(&file);
        assert_only_the_block_was_added(&before, &entries_of(&file));
        assert_eq!(saved_block(&file, "social-media"), Some(drafted.block.clone()), "{label}");
    }

    // Every zip of the phone's corpus that the phone reads takes the block and keeps its other files.
    let (mut added, mut full, mut skipped) = (0, 0, 0);
    for item in load_corpus().cases.iter().filter(|item| item["expect"].get("read").is_some()) {
        let (name, bytes) = (item["name"].as_str().unwrap_or("?"), case_bytes(item, Method::Deflate));
        let Ok(opened) = leko_core::open_zip(&bytes) else { skipped += 1; continue };
        let (_folder, file) = write_into("corpus", &bytes);
        let run = add(&file, &[&FLAGS[..], &["--write"]].concat());
        if opened.files.len() >= 500 {
            assert_eq!((run.code, run.out.as_str(), std::fs::read(&file).unwrap()), (1, "", bytes), "{name}: a zip with the most files takes no more");
            full += 1;
            continue;
        }
        assert_eq!((run.code, run.err.as_str()), (0, ""), "{name}: {}", run.out);
        assert_core_reads(&file);
        assert_only_the_block_was_added(&opened.files, &entries_of(&file));
        added += 1;
    }
    assert!(added >= 1, "the corpus was used: {added} zips took the block, {full} were full, {skipped} could not be opened");
}
