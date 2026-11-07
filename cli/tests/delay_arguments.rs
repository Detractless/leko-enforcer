//! Wrong or missing words after `switch-off` or `settle` print the problem and the usage to stderr and
//! exit 2 (AC-008), before any file is read; nothing is written. The zone is `--zone`, else the
//! computer's, and is never guessed: none at all, or an unusable one, is a refusal (exit 1), and a
//! zone the user named that does not exist is a wrong word (exit 2).

mod delay_runs;
#[path = "../../core/tests/delay_sets/mod.rs"]
mod delay_sets;
mod run_capture;
mod scratch_folder;
#[path = "../../core/tests/zip_writer/mod.rs"]
mod zip_writer;

use delay_runs::{one_line, settle_in, snapshot, switch_off_in, write_zip, NOW, ZONE};
use delay_sets::{locked_block, wait};
use leko_cli::session::session_arguments::USAGE;
use run_capture::{moment, run_in_zone, Outcome};
use scratch_folder::Scratch;
use serde_json::Value;
use std::ffi::OsString;

/// `leko COMMAND WORDS...` at NOW on a New York computer; `FILE` in a word stands for `file`.
fn words(command: &str, file: &str, words: &[&str]) -> Outcome {
    let mut args: Vec<OsString> = vec![command.into()];
    args.extend(words.iter().map(|word| OsString::from(if *word == "FILE" { file } else { word })));
    run_in_zone(args, moment(NOW), Some(ZONE))
}

fn assert_not_understood(run: &Outcome, problem: &str, what: &str) {
    assert_eq!((run.code, run.out.as_str()), (2, ""), "{what}: {}", run.err);
    assert_eq!(run.err, format!("{problem}\n{USAGE}\n"), "{what}");
}

#[test]
fn bad_arguments_print_usage_and_exit_two_for_both_commands() {
    let folder = Scratch::new("arguments");
    let file = write_zip(folder.path(), "s.zip", &[locked_block("a", Value::Null)], &[]);
    let file = file.to_str().expect("a path of text");
    let before = snapshot(folder.path());
    let missing_file = folder.path().join("missing.zip");
    let missing_file = missing_file.to_str().expect("a path of text");

    let switch_off_cases: [(&str, &[&str], &str); 11] = [
        ("no file", &["--id", "a"], "No settings file was given."),
        ("no --id", &["FILE"], "Say which block to switch off: --id ID."),
        ("--id twice", &["FILE", "--id", "a", "--id", "b"], "--id was given twice."),
        ("--id with nothing after it", &["FILE", "--id"], "--id needs a block id after it."),
        ("--id followed by a flag", &["FILE", "--id", "--write"], "--id needs a block id after it."),
        ("an unknown flag", &["FILE", "--id", "a", "--force"], "\"--force\" is not a flag of this command."),
        ("a stray word", &["FILE", "extra", "--id", "a"], "Only one settings file can be given."),
        ("--write twice", &["FILE", "--id", "a", "--write", "--write"], "--write was given twice."),
        ("--zone with nothing after it", &["FILE", "--id", "a", "--zone"], "--zone needs a zone name after it."),
        ("--zone twice", &["FILE", "--id", "a", "--zone", "UTC", "--zone", "UTC"], "--zone was given twice."),
        ("an empty --zone", &["FILE", "--id", "a", "--zone", ""], "--zone needs a zone name after it."),
    ];
    for (what, args, problem) in switch_off_cases {
        assert_not_understood(&words("switch-off", file, args), problem, &format!("switch-off, {what}"));
    }
    let settle_cases: [(&str, &[&str], &str); 8] = [
        ("no file", &[], "No settings file was given."),
        ("--id", &["FILE", "--id", "a"], "--id is only for switch-off: settle lands every change that is due."),
        ("an unknown flag", &["FILE", "--force"], "\"--force\" is not a flag of this command."),
        ("a stray word", &["FILE", "extra"], "Only one settings file can be given."),
        ("--write twice", &["FILE", "--write", "--write"], "--write was given twice."),
        ("--zone with nothing after it", &["FILE", "--zone"], "--zone needs a zone name after it."),
        ("--zone twice", &["FILE", "--zone", "UTC", "--zone", "UTC"], "--zone was given twice."),
        ("an empty --zone", &["FILE", "--zone", ""], "--zone needs a zone name after it."),
    ];
    for (what, args, problem) in settle_cases {
        assert_not_understood(&words("settle", file, args), problem, &format!("settle, {what}"));
    }
    // The words are read before any file: a missing settings file does not hide a wrong word.
    assert_not_understood(&words("switch-off", missing_file, &["FILE"]), "Say which block to switch off: --id ID.", "a missing file and no --id");
    assert_not_understood(&words("settle", missing_file, &["FILE", "--id", "a"]), "--id is only for switch-off: settle lands every change that is due.", "a missing file and --id");
    assert_eq!(snapshot(folder.path()), before, "wrong words touched nothing");

    // A zone the user named that does not exist is a wrong word, not UTC and not the computer's zone.
    for (command, args) in [("switch-off", &["FILE", "--id", "a", "--zone", "Nowhere/Land"][..]), ("settle", &["FILE", "--zone", "Nowhere/Land"])] {
        let run = words(command, file, args);
        assert_eq!((run.code, run.out.as_str()), (2, ""), "{command}: {}", run.err);
        assert!(run.err.contains("Nowhere/Land") && run.err.ends_with(&format!("{USAGE}\n")), "{command}: {}", run.err);
        assert_eq!(snapshot(folder.path()), before, "{command}: an unknown zone saved nothing");
    }
}

#[test]
fn the_zone_is_the_flag_else_the_computers_and_is_never_guessed() {
    let folder = Scratch::new("arguments-zone");
    // A wait of 24 hours from NOW: 10:00 on the 3rd in New York, 00:00 on the 4th in Tokyo.
    let file = write_zip(folder.path(), "s.zip", &[locked_block("a", wait(24))], &[]);
    let before = snapshot(folder.path());
    let new_york = "may land from 2026-11-03T10:00:00-05:00 (America/New_York)";
    let tokyo = "may land from 2026-11-04T00:00:00+09:00 (Asia/Tokyo)";

    let shown = |run: &Outcome, moment: &str, what: &str| {
        assert_eq!((run.code, run.err.as_str()), (0, ""), "{what}");
        assert!(run.out.contains(moment), "{what}: {}", run.out);
    };
    shown(&switch_off_in(&file, &["--id", "a"], NOW, Some("Asia/Tokyo")), tokyo, "the computer's zone is used");
    shown(&switch_off_in(&file, &["--id", "a", "--zone", "America/New_York"], NOW, Some("Asia/Tokyo")), new_york, "--zone beats the computer's zone");
    shown(&switch_off_in(&file, &["--id", "a", "--zone", "Asia/Tokyo"], NOW, None), tokyo, "--zone needs no computer zone");
    shown(&switch_off_in(&file, &["--id", "a", "--zone", "America/New_York"], NOW, Some("Not/AZone")), new_york, "--zone beats an unusable computer zone");

    // No zone to judge in is a refusal that says how to give one, for both commands, and nothing is saved.
    for (command, run) in [
        ("switch-off with no computer zone", switch_off_in(&file, &["--id", "a", "--write"], NOW, None)),
        ("switch-off with an unusable computer zone", switch_off_in(&file, &["--id", "a", "--write"], NOW, Some("Not/AZone"))),
        ("settle with no computer zone", settle_in(&file, &["--write"], NOW, None)),
        ("settle with an unusable computer zone", settle_in(&file, &["--write"], NOW, Some("Not/AZone"))),
    ] {
        assert_eq!((run.code, run.out.as_str()), (1, ""), "{command}: {}", run.err);
        let line = one_line(&run.err);
        assert!(line.contains("--zone") && line.ends_with(". Nothing was saved.\n"), "{command}: {line}");
        assert!(!line.contains("Usage"), "{command}: a refusal is not a wrong word");
        assert_eq!(snapshot(folder.path()), before, "{command}: nothing was saved");
    }
}
