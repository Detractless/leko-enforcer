//! The real `leko` program, started as a process, keeps results on stdout and errors on stderr with
//! the codes 0, 1 and 2, and does not panic when an output is closed; after a `--write` the block is
//! still saved in full (AC-013). A `--for` block ends the asked length after the real clock (AC-014).

mod add_block_runs;
mod run_capture;
mod scratch_folder;
mod settings_builder;

use add_block_runs::{assert_core_reads, entries_of, saved_block};
use leko_cli::run;
use leko_core::sessions::utc_moment::parse_utc_moment;
use run_capture::{run_with_closed, Broken, Closed, Outcome};
use scratch_folder::Scratch;
use serde_json::json;
use settings_builder::{always, block, write_settings};
use std::ffi::OsString;
use std::process::{Command, Output, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

const MINUTE_MS: i64 = 60_000;

fn outcome_of(output: Output) -> Outcome {
    Outcome {
        code: output.status.code().expect("the program ended by itself"),
        out: String::from_utf8(output.stdout).expect("stdout is text"),
        err: String::from_utf8(output.stderr).expect("stderr is text"),
    }
}

fn command(args: &[&str]) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_leko"));
    command.args(args).stdin(Stdio::null()).env("TZ", "UTC");
    command
}

fn leko(args: &[&str]) -> Outcome {
    outcome_of(command(args).output().expect("the program starts"))
}

fn clock_ms() -> i64 {
    i64::try_from(SystemTime::now().duration_since(UNIX_EPOCH).expect("a clock after 1970").as_millis()).expect("a moment that fits")
}

/// Runs the program with one output pointing at a pipe that nothing reads.
fn leko_with_closed(args: &[&str], closed: Closed) -> Outcome {
    outcome_of(run_with_closed(command(args), closed))
}

fn run_into_broken(args: &[&str], out: &mut Broken) -> (i32, String) {
    let args: Vec<OsString> = args.iter().map(OsString::from).collect();
    let mut err = Vec::new();
    let code = run(&args, 1_000_000_000_000, Some("UTC"), out, &mut err);
    (code, String::from_utf8(err).expect("stderr is text"))
}

fn fresh_settings(label: &str) -> (Scratch, String) {
    let folder = Scratch::new(label);
    let file = write_settings(folder.path(), "settings.zip", &[block("old", json!({"apps": ["com.old.app"]}), json!([always("r", &[])]))]);
    let text = file.to_str().expect("a text path").to_string();
    (folder, text)
}

#[test]
fn the_real_program_keeps_streams_and_codes_apart_and_survives_a_closed_pipe() {
    let (folder, file) = fresh_settings("real-add");
    let request = ["--name", "Social Media", "--app", "com.example.app", "--always"];
    let with = |extra: &[&'static str]| -> Vec<&str> { [&["add-block", file.as_str()][..], &request, extra].concat() };

    // Success: a draft and then the saved block go to stdout alone; the file changes only with --write.
    let before = std::fs::read(&file).unwrap();
    let draft = leko(&with(&[]));
    assert_eq!((draft.code, draft.err.as_str()), (0, ""), "{}", draft.err);
    assert!(draft.out.starts_with("Draft of a new block called \"Social Media\""), "{}", draft.out);
    assert_eq!(std::fs::read(&file).unwrap(), before, "a draft saves nothing");
    let saved = leko(&with(&["--write"]));
    assert_eq!((saved.code, saved.err.as_str()), (0, ""), "{}", saved.err);
    assert!(saved.out.starts_with("Saved the block \"social-media\""), "{}", saved.out);
    assert!(saved_block(file.as_ref(), "social-media").is_some());
    let after_save = std::fs::read(&file).unwrap();

    // A request understood but not possible is exit 1 (one line on stderr, nothing on stdout); words
    // not understood are exit 2 (the usage on stderr, nothing on stdout).
    let taken = leko(&with(&["--write"]));
    let missing = leko(&["add-block", "/no/such/settings.zip", "--name", "X", "--app", "com.a", "--always"]);
    let bad_package = leko(&["add-block", &file, "--name", "Y", "--app", "has space", "--always"]);
    for refused in [&taken, &missing, &bad_package] {
        assert_eq!((refused.code, refused.out.as_str(), refused.err.lines().count()), (1, "", 1), "{}", refused.err);
        assert!(!refused.err.contains("Usage"), "{}", refused.err);
    }
    let not_understood = leko(&["add-block", &file]);
    assert_eq!((not_understood.code, not_understood.out.as_str()), (2, ""));
    assert!(not_understood.err.contains("Usage: leko add-block") || not_understood.err.contains("leko add-block FILE.zip"), "{}", not_understood.err);
    assert_eq!(std::fs::read(&file).unwrap(), after_save, "nothing above changed the file");

    // stdout closed: nothing can be shown, so exit 1 and a sentence on stderr, never a panic. (These
    // drafts are of blocks not in the file yet, so the id is no reason to refuse them.)
    let new_draft = |name: &'static str| -> Vec<&str> { vec!["add-block", file.as_str(), "--name", name, "--app", "com.new.app", "--always"] };
    let draft = leko_with_closed(&new_draft("Draft One"), Closed::Stdout);
    assert_eq!(draft.code, 1, "{}", draft.err);
    assert!(draft.err.contains("could not be written") && !draft.err.contains("panicked"), "{}", draft.err);
    assert_eq!(std::fs::read(&file).unwrap(), after_save);
    // A refusal and a usage problem write nothing to stdout, so a closed stdout does not change them.
    assert_eq!(leko_with_closed(&with(&["--write"]), Closed::Stdout).code, 1, "an id taken");
    assert_eq!(leko_with_closed(&["add-block", &file], Closed::Stdout).code, 2);
    // stderr closed: the codes are the same and nothing panics; a success does not need stderr.
    assert_eq!(leko_with_closed(&with(&["--write"]), Closed::Stderr).code, 1);
    assert_eq!(leko_with_closed(&["add-block", &file], Closed::Stderr).code, 2);
    let made = leko_with_closed(&new_draft("Draft Two"), Closed::Stderr);
    assert_eq!((made.code, made.err.as_str()), (0, ""));
    assert!(made.out.starts_with("Draft of a new block"), "{}", made.out);

    // stdout closed AFTER a --write: the block is saved in full anyway, with the same plain sentence.
    let other = ["add-block", file.as_str(), "--name", "Late Night", "--site", "example.org", "--always", "--write"];
    let late = leko_with_closed(&other, Closed::Stdout);
    assert_eq!(late.code, 1, "{}", late.err);
    assert!(late.err.contains("The result could not be written to the output.") && !late.err.contains("panicked"), "{}", late.err);
    assert_core_reads(file.as_ref());
    let block = saved_block(file.as_ref(), "late-night").expect("the block was saved before the output failed");
    assert_eq!(block["covers"], json!({"sites": ["example.org"]}));
    assert_eq!(entries_of(file.as_ref()).len(), 5, "GUIDE.md, general.json, and three blocks");
    assert_eq!(folder.names(), ["settings.zip"], "no temporary file");

    // The same in this process, with outputs that fail when written or when flushed.
    for (index, (write_fails, flush_fails)) in [(true, false), (false, true), (true, true)].into_iter().enumerate() {
        let name = format!("Broken {index}");
        let args = ["add-block", file.as_str(), "--name", name.as_str(), "--app", "com.broken.app", "--always", "--write"];
        let (code, err) = run_into_broken(&args, &mut Broken { write_fails, flush_fails });
        assert_eq!(code, 1, "write_fails={write_fails} flush_fails={flush_fails}");
        assert!(err.contains("could not be written"), "{err}");
        assert!(saved_block(file.as_ref(), &format!("broken-{index}")).is_some(), "saved before the output failed");
    }
    assert_core_reads(file.as_ref());
    assert_eq!(run_into_broken(&["add-block"], &mut Broken { write_fails: true, flush_fails: true }).0, 2);
    let (_folder, file) = fresh_settings("real-add-good-output");
    let args = ["add-block", file.as_str(), "--name", "Fine", "--app", "com.fine.app", "--always"];
    assert_eq!(run_into_broken(&args, &mut Broken { write_fails: false, flush_fails: false }), (0, String::new()), "an output that works is not caught by these checks");
}

#[test]
fn a_timed_block_ends_the_asked_length_after_the_real_clock() {
    let (_folder, file) = fresh_settings("real-timed");
    for (flag, minutes) in [("90m", 90), ("2h", 120), ("45", 45)] {
        let name = format!("Focus {flag}");
        let (before, run) = (clock_ms(), leko(&["add-block", &file, "--name", &name, "--app", "com.example.app", "--for", flag, "--write"]));
        let after = clock_ms();
        assert_eq!((run.code, run.err.as_str()), (0, ""), "{}", run.err);
        let id = format!("focus-{flag}");
        let saved = saved_block(file.as_ref(), &id).expect("the block was saved");
        let ends = parse_utc_moment(saved["rules"][0]["endsAt"].as_str().expect("an end")).expect("a UTC moment");
        // The end is a whole second, rounded up from the clock plus the length: never early, and never more
        // than a second late, so well within a minute of what the real clock read.
        assert!(ends >= before + minutes * MINUTE_MS && ends <= after + minutes * MINUTE_MS + 1_000, "{flag}: ends at {ends}, the clock read {before} to {after}");
        assert!(ends - (before + minutes * MINUTE_MS) < MINUTE_MS, "{flag}: within a minute");
        assert_eq!(saved["lock"]["at"], saved["rules"][0]["endsAt"], "{flag}: the seal and the rule end together");
    }
}
