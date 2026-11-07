//! A block is never overwritten (AC-006): an id already in the file is refused with its name in the
//! sentence, whether the old block is on or off and whichever name made the id; a second identical
//! `--write` saves nothing and leaves the file exactly as the first left it.

mod add_block_runs;
mod run_capture;
mod scratch_folder;
mod settings_builder;

use add_block_runs::{add, entries_of, plain_and, saved_block, APP};
use scratch_folder::Scratch;
use serde_json::json;
use settings_builder::{always, block, block_with, write_settings};

#[test]
fn an_existing_block_is_never_overwritten_and_a_second_write_changes_nothing() {
    let old_apps = json!({"apps": ["com.old.app"]});
    let on = block("social-media", old_apps.clone(), json!([always("r", &[])]));
    let off = block_with("social-media", old_apps, json!([always("r", &[])]), json!({"on": false}));
    for (label, old) in [("on", on), ("off", off)] {
        // Every name that makes the id "social-media" is refused, in the draft and with --write.
        for name in ["Social Media", "social-media", "SOCIAL_MEDIA", "  Social   Media!! "] {
            for write in [false, true] {
                let folder = Scratch::new("repeat");
                let file = write_settings(folder.path(), "settings.zip", &[old.clone()]);
                let (bytes, entries) = (std::fs::read(&file).unwrap(), entries_of(&file));
                let flags = [&["--name", name, "--app", APP, "--always"][..], if write { &["--write"] } else { &[] }].concat();
                let run = add(&file, &flags);
                let what = format!("{label}, {name:?}, write {write}");
                assert_eq!((run.code, run.out.as_str()), (1, ""), "{what}: {}", run.err);
                assert!(run.err.contains("\"social-media\""), "{what}: the sentence names the id: {}", run.err);
                assert_eq!(run.err.lines().count(), 1, "{what}: {}", run.err);
                assert_eq!(std::fs::read(&file).unwrap(), bytes, "{what}: the file is as it was");
                assert_eq!(entries_of(&file), entries, "{what}");
                assert_eq!(folder.names(), ["settings.zip"], "{what}");
            }
        }
    }

    // A second identical --write saves nothing: the first one saved once, the second leaves the file as it is.
    let folder = Scratch::new("twice");
    let file = write_settings(folder.path(), "settings.zip", &[block("old", json!({"apps": ["com.old.app"]}), json!([always("r", &[])]))]);
    let flags = plain_and(&["--write"]);
    let first = add(&file, &flags);
    assert_eq!((first.code, first.err.as_str()), (0, ""), "{}", first.out);
    let (after_first, entries) = (std::fs::read(&file).unwrap(), entries_of(&file));
    let second = add(&file, &flags);
    assert_eq!((second.code, second.out.as_str()), (1, ""), "{}", second.err);
    assert!(second.err.contains("\"social-media\"") && second.err.contains("nothing was added"), "{}", second.err);
    assert_eq!(std::fs::read(&file).unwrap(), after_first, "the second write leaves the file exactly as the first left it");
    assert_eq!(entries_of(&file), entries);
    assert_eq!(entries.iter().filter(|entry| entry.0 == "blocks/social-media.json").count(), 1, "one block file, not two");
    assert_eq!(folder.names(), ["settings.zip"]);

    // A different name still works next to it, and the first block is untouched.
    let before = saved_block(&file, "social-media");
    let run = add(&file, &["--name", "Social Media 2", "--site", "example.org", "--always", "--write"]);
    assert_eq!((run.code, run.err.as_str()), (0, ""), "{}", run.out);
    assert_eq!(saved_block(&file, "social-media"), before);
    assert!(saved_block(&file, "social-media-2").is_some());
}
