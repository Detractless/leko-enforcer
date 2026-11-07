//! The door and the message of a new block follow fixed defaults (AC-005): a wait of 30 seconds, no
//! way in for `--wall` and for every timed block, a wait of 0 to 600 seconds on request, a message of
//! 1 to 300 characters, and every new block switched on.

mod add_block_runs;
mod run_capture;
mod scratch_folder;
mod settings_builder;

use add_block_runs::{add, plain, plain_and, saved_block, APP};
use run_capture::Outcome;
use scratch_folder::Scratch;
use serde_json::{json, Value};
use settings_builder::{always, block, write_settings};

/// Adds a block with `flags` and `--write` to a fresh file; returns the run and the saved block.
fn added(flags: &[&str]) -> (Outcome, Option<Value>) {
    let folder = Scratch::new("door");
    let file = write_settings(folder.path(), "settings.zip", &[block("old", json!({"apps": ["com.old.app"]}), json!([always("r", &[])]))]);
    let run = add(&file, &[flags, &["--write"]].concat());
    let saved = saved_block(&file, "social-media");
    assert_eq!(saved.is_some(), run.code == 0, "saved exactly when it worked: {}{}", run.out, run.err);
    (run, saved)
}

fn door_of(flags: &[&str]) -> Value {
    let (run, saved) = added(flags);
    assert_eq!((run.code, run.err.as_str()), (0, ""), "{flags:?}: {}", run.out);
    saved.unwrap()["door"].clone()
}

fn with_message(text: &str) -> Vec<&str> {
    plain_and(&["--message", text])
}

fn refused(flags: &[&str]) -> String {
    let (run, saved) = added(flags);
    assert_eq!((run.code, run.out.as_str(), saved), (1, "", None), "{flags:?}");
    assert_eq!(run.err.lines().count(), 1, "one sentence: {}", run.err);
    run.err
}

#[test]
fn the_door_defaults_to_a_30_second_wait_and_a_timed_block_has_no_way_in() {
    let wait = |seconds: i64| json!({"kind": "wait", "seconds": seconds});
    let wall = json!({"kind": "wall"});
    assert_eq!(door_of(&plain()), wait(30), "no door flag: the standard wait");
    assert_eq!(door_of(&plain_and(&["--wall"])), wall);
    for seconds in [0, 1, 5, 600] {
        assert_eq!(door_of(&plain_and(&["--wait", &seconds.to_string()])), wait(seconds), "--wait {seconds}");
    }
    for seconds in ["601", "9999"] {
        let sentence = refused(&plain_and(&["--wait", seconds]));
        assert!(sentence.contains("0 to 600"), "{sentence}");
    }

    // A wait door is a way in, so the block carries a pass; a wall has neither.
    let (_, waiting) = added(&plain());
    assert_eq!(waiting.unwrap()["pass"], json!({"length": "minutes", "minutes": 10, "perDay": 3}));
    let (_, walled) = added(&plain_and(&["--wall"]));
    assert!(walled.unwrap().get("pass").is_none(), "a wall has no pass");

    // A timed block always has a wall; a wait on it is refused, a wall on it is fine.
    let timed = ["--name", "Social Media", "--app", APP];
    for extra in [&["--for", "90m"][..], &["--for", "90m", "--wall"]] {
        let (run, saved) = added(&[&timed[..], extra].concat());
        assert_eq!((run.code, run.err.as_str()), (0, ""), "{extra:?}: {}", run.out);
        let saved = saved.unwrap();
        assert_eq!(saved["door"], wall, "{extra:?}");
        assert!(saved.get("pass").is_none(), "{extra:?}: a timed block has no pass");
    }
    let sentence = refused(&[&timed[..], &["--for", "90m", "--wait", "30"]].concat());
    assert!(sentence.contains("timed block"), "{sentence}");
    assert!(refused(&[&timed[..], &["--for", "90m", "--wait", "0"]].concat()).contains("timed block"), "even a wait of nothing is a way in");

    // The message: 1 to 300 characters, no control character; the block is switched on.
    let (run, saved) = added(&with_message("Back to work"));
    assert_eq!(run.code, 0, "{}", run.out);
    let saved = saved.unwrap();
    assert_eq!((saved["message"].as_str(), saved["on"].as_bool()), (Some("Back to work"), Some(true)));
    assert_eq!(saved["name"], "Social Media");
    let longest = "a".repeat(300);
    assert_eq!(added(&with_message(&longest)).1.unwrap()["message"], longest.as_str());
    assert_eq!(added(&with_message("a")).1.unwrap()["message"], "a");
    let accents = "é".repeat(300);
    assert_eq!(added(&with_message(&accents)).1.unwrap()["message"], accents.as_str(), "300 accented letters are 300 characters");
    for (label, text) in [("301 letters", "a".repeat(301)), ("empty", String::new()), ("a bell", "stay\u{7}focused".to_string()), ("a new line", "two\nlines".to_string()), ("a tab", "a\tb".to_string())] {
        let sentence = refused(&with_message(&text));
        assert!(sentence.to_lowercase().contains("message"), "{label}: {sentence}");
        if matches!(label, "301 letters" | "empty") {
            assert!(sentence.starts_with("A message is 1 to 300 characters, not "), "{label}: the drafter's own sentence, not a later catch: {sentence}");
        }
    }
    let (run, saved) = added(&plain());
    assert_eq!(run.code, 0);
    let saved = saved.unwrap();
    assert!(saved.get("message").is_none(), "no message flag, no message");
    assert_eq!(saved["on"], true, "every new block is switched on");
}
