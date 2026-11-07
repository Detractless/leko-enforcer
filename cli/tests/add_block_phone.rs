//! The draft and the saved report say when the phone's Enforcer (0.3) could not read the file
//! (AC-008): the warning is there exactly when reading the result in the phone's mode is refused,
//! with the core's reason, and the block is still drafted and, with `--write`, saved. What the phone
//! would do is worked out here by reading the saved zip in the phone's mode with the core.

mod add_block_runs;
mod run_capture;
mod scratch_folder;
mod settings_builder;

use add_block_runs::{add, saved_block, APP};
use leko_core::io::zip_check::{check_zip_in_mode, ZipVerdict};
use leko_core::models::specs::ReadMode;
use scratch_folder::Scratch;
use serde_json::json;
use settings_builder::{always, block, settings_bytes, write_settings};
use std::collections::HashSet;

const WARNING: &str = "Warning: the phone's Enforcer 0.3 could not read this file, so it would not sync there. ";

fn phone_reason(bytes: &[u8]) -> Option<String> {
    match check_zip_in_mode(bytes, &HashSet::new(), ReadMode::Phone) {
        ZipVerdict::Refused(refusal) => Some(refusal.sentence),
        ZipVerdict::Read(_) => None,
    }
}

#[test]
fn the_warning_appears_exactly_when_the_phone_would_refuse_the_file() {
    let plain_old = block("old", json!({"apps": ["com.old.app"]}), json!([always("r", &[])]));
    let site_old = block("old", json!({"sites": ["old.example.org"]}), json!([always("r", &[])]));
    assert_eq!(phone_reason(&settings_bytes(&[plain_old.clone()])), None, "the phone reads the plain old block");
    let requests: [(&str, &[&str], bool); 9] = [
        ("an app, always", &["--app", APP, "--always"], false),
        ("an app, open hours", &["--app", APP, "--open-hours", "09:00-17:00"], false),
        ("an app, open hours on some days", &["--app", APP, "--open-hours", "09:00-17:00,19:00-21:00", "--days", "mon,tue"], false),
        ("an app, a wall", &["--app", APP, "--always", "--wall"], false),
        ("a site", &["--site", "example.org", "--always"], true),
        ("an app and a site", &["--app", APP, "--site", "example.org", "--always"], true),
        ("a site, open hours", &["--site", "example.org", "--open-hours", "09:00-17:00"], true),
        ("an app, timed", &["--app", APP, "--for", "90m"], true),
        ("a site, timed", &["--site", "example.org", "--for", "2h"], true),
    ];
    for old in [Some(&plain_old), None] {
        for (what, flags, warns) in requests {
            let blocks: Vec<_> = old.cloned().into_iter().collect();
            let folder = Scratch::new("phone");
            let file = if blocks.is_empty() {
                let path = folder.path().join("settings.zip");
                std::fs::write(&path, settings_bytes(&[])).unwrap();
                path
            } else {
                write_settings(folder.path(), "settings.zip", &blocks)
            };
            let flags = [&["--name", "Test Block"][..], flags].concat();

            // The draft: the block is shown, and the warning is there only when the phone would refuse.
            let draft = add(&file, &flags);
            assert_eq!((draft.code, draft.err.as_str()), (0, ""), "{what}: {}", draft.out);
            assert!(draft.out.starts_with("Draft of a new block called \"Test Block\""), "{what}: {}", draft.out);
            assert_eq!(draft.out.contains("Warning:"), warns, "{what}: {}", draft.out);

            // The save: the same words about the same file, and the block is saved in both cases.
            let saved = add(&file, &[&flags[..], &["--write"]].concat());
            assert_eq!((saved.code, saved.err.as_str()), (0, ""), "{what}: {}", saved.out);
            assert!(saved_block(&file, "test-block").is_some(), "{what}: saved even with a warning");
            let reason = phone_reason(&std::fs::read(&file).unwrap());
            assert_eq!(reason.is_some(), warns, "{what}: the test's idea of the phone agrees with the core");
            for (label, text) in [("draft", &draft.out), ("saved", &saved.out)] {
                match &reason {
                    Some(why) => assert!(text.lines().any(|line| line.starts_with(WARNING) && line.contains(why.as_str())), "{what}, {label}: the core's reason {why:?} is in the warning:\n{text}"),
                    None => assert!(!text.contains("Warning") && !text.contains("Enforcer 0.3"), "{what}, {label}: {text}"),
                }
            }
        }
    }

    // A file the phone already could not read keeps the warning for a plain app block, with the reason.
    let folder = Scratch::new("phone-old");
    let file = write_settings(folder.path(), "settings.zip", &[site_old]);
    let run = add(&file, &["--name", "Test Block", "--app", APP, "--always"]);
    assert_eq!((run.code, run.err.as_str()), (0, ""), "{}", run.out);
    let reason = phone_reason(&settings_bytes(&[block("old", json!({"sites": ["old.example.org"]}), json!([always("r", &[])]))])).unwrap();
    assert!(run.out.lines().any(|line| line.starts_with(WARNING) && line.contains(&reason)), "{}", run.out);
}
