//! Without `--write` the command shows one plain draft and saves nothing (AC-001): the block's name and
//! id, what it covers, the rule in words, the door, that never-blocked apps are not checked yet,
//! "Nothing was saved", and the same command with `--write`. The file's bytes, its folder and the
//! program's own home and temporary folders are as they were.

mod add_block_runs;
mod run_capture;
mod scratch_folder;
mod settings_builder;

use add_block_runs::{add, plain, APP};
use scratch_folder::Scratch;
use serde_json::json;
use settings_builder::{always, block, write_settings};
use std::process::{Command, Stdio};

fn existing_blocks() -> Vec<serde_json::Value> {
    vec![block("old", json!({"apps": ["com.old.app"]}), json!([always("r", &[])]))]
}

#[test]
fn without_the_write_flag_the_draft_is_shown_and_nothing_is_saved() {
    let folder = Scratch::new("draft");
    let file = write_settings(folder.path(), "settings.zip", &existing_blocks());
    let (before, names, stamp) = (std::fs::read(&file).unwrap(), folder.names(), std::fs::metadata(&file).unwrap().modified().unwrap());

    let always_run = add(&file, &plain());
    assert_eq!((always_run.code, always_run.err.as_str()), (0, ""), "{}", always_run.out);
    let text = &always_run.out;
    for want in ["\"Social Media\"", "\"social-media\"", APP, "closed all day", "Every day", "30 seconds", "never blocked are not checked yet", "Nothing was saved", "--write"] {
        assert!(text.contains(want), "the draft lacks {want:?}: {text}");
    }
    let last = text.lines().last().unwrap();
    assert!(last.contains(&format!("leko add-block {} --name 'Social Media' --app {APP} --always --write", file.display())), "the last line is the same command with --write: {last}");

    let hours_flags = ["--name", "Social Media", "--app", APP, "--site", "example.com", "--open-hours", "09:00-12:00,13:00-17:00", "--days", "mon,tue"];
    let hours = add(&file, &hours_flags);
    assert_eq!((hours.code, hours.err.as_str()), (0, ""), "{}", hours.out);
    for want in ["the site \"example.com\"", "On mon, tue it is open only during 09:00-12:00, 13:00-17:00", "closed at all other times", "does not close anything"] {
        assert!(hours.out.contains(want), "the draft lacks {want:?}: {}", hours.out);
    }

    let timed = add(&file, &["--name", "Study", "--app", APP, "--for", "90m", "--wall"]);
    assert_eq!((timed.code, timed.err.as_str()), (0, ""), "{}", timed.out);
    for want in ["closed for 90 minutes, until 2026-11-05T16:30:00Z (UTC)", "no way in"] {
        assert!(timed.out.contains(want), "the draft lacks {want:?}: {}", timed.out);
    }

    // Nothing changed: bytes, folder, time stamp.
    assert_eq!(std::fs::read(&file).unwrap(), before);
    assert_eq!(folder.names(), names);
    assert_eq!(std::fs::metadata(&file).unwrap().modified().unwrap(), stamp);

    // A file name with an escape character is written out, not sent to the terminal.
    let odd = write_settings(folder.path(), "odd\u{1b}[31m.zip", &existing_blocks());
    let shown = add(&odd, &plain());
    assert_eq!(shown.code, 0, "{}", shown.err);
    assert!(!shown.out.contains('\u{1b}') && shown.out.contains("odd\\u{1b}[31m.zip"), "{:?}", shown.out);
}

#[test]
fn the_real_program_leaves_its_home_and_temporary_folders_empty() {
    let folder = Scratch::new("draft-real");
    let (home, temp) = (Scratch::new("draft-home"), Scratch::new("draft-temp"));
    let file = write_settings(folder.path(), "settings.zip", &existing_blocks());
    let before = std::fs::read(&file).unwrap();
    let run = Command::new(env!("CARGO_BIN_EXE_leko"))
        .args(["add-block", file.to_str().unwrap(), "--name", "Social Media", "--app", APP, "--always"])
        .env("HOME", home.path())
        .env("TMPDIR", temp.path())
        .current_dir(home.path())
        .stdin(Stdio::null())
        .output()
        .expect("the program starts");
    assert_eq!(run.status.code(), Some(0), "{}", String::from_utf8_lossy(&run.stderr));
    assert!(String::from_utf8_lossy(&run.stdout).contains("Nothing was saved"));
    assert_eq!((home.names(), temp.names(), folder.names(), std::fs::read(&file).unwrap()), (vec![], vec![], vec!["settings.zip".to_string()], before));
}
