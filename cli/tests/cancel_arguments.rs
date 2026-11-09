//! Wrong or missing words after `leko cancel` make nothing (AC-007): one plain sentence and the usage
//! line (which now lists `leko cancel`) on stderr, nothing on stdout, exit 2, and no file touched. A
//! missing `--id` never cancels the first record, and an unknown flag (including `--zone`, which a
//! cancel has no use for) is never ignored.

mod parked_change_support;
use parked_change_support::*;

use delay_runs::{record, snapshot, write_waiting, write_zip};
use delay_sets::{locked_block, wait};
use leko_cli::session::session_arguments::USAGE;
use run_capture::run_at;
use scratch_folder::Scratch;

#[test]
fn bad_arguments_print_usage_and_exit_two() {
    let folder = Scratch::new("cancel-arguments");
    let file = write_zip(folder.path(), "s.zip", &[locked_block("a", wait(24)), locked_block("b", wait(24))], &[]);
    write_waiting(&file, &[record("a", "2026-11-02T14:00:00Z", "2026-11-03T14:00:00Z"), record("b", "2026-11-02T13:00:00Z", "2026-11-03T13:00:00Z")]);
    let before = snapshot(folder.path());
    let f = file.to_str().expect("a text path");

    let flag = |name: &str| format!("\"{name}\" is not a flag of this command.");
    let cases: Vec<(Vec<&str>, String)> = vec![
        (vec!["cancel"], "No settings file was given.".to_string()),
        (vec!["cancel", "--id", "a"], "No settings file was given.".to_string()),
        (vec!["cancel", "--write"], "No settings file was given.".to_string()),
        (vec!["cancel", f], "Say which block's waiting change to cancel: --id ID.".to_string()),
        (vec!["cancel", f, "--write"], "Say which block's waiting change to cancel: --id ID.".to_string()),
        (vec!["cancel", f, "--id"], "--id needs a block id after it.".to_string()),
        (vec!["cancel", f, "--id", "--write"], "--id needs a block id after it.".to_string()),
        (vec!["cancel", f, "--id", ""], "--id needs a block id after it.".to_string()),
        (vec!["cancel", f, "--id", "a", "--id", "b"], "--id was given twice.".to_string()),
        (vec!["cancel", f, "--id", "a", "--id", "a", "--write"], "--id was given twice.".to_string()),
        (vec!["cancel", f, "--id", "a", "--write", "--write"], "--write was given twice.".to_string()),
        (vec!["cancel", f, "--id", "a", "--zone", "UTC"], flag("--zone")),
        (vec!["cancel", f, "--id", "a", "--zone"], flag("--zone")),
        (vec!["cancel", f, "--id", "a", "--json"], flag("--json")),
        (vec!["cancel", f, "--id", "a", "--wat"], flag("--wat")),
        (vec!["cancel", f, "--id", "a", "-x", "--write"], flag("-x")),
        (vec!["cancel", f, "other.zip", "--id", "a"], "Only one settings file can be given.".to_string()),
        (vec!["cancel", f, "--id", "a", "other.zip", "--write"], "Only one settings file can be given.".to_string()),
    ];
    for (words, sentence) in &cases {
        let run = run_at(words, 0);
        assert_eq!(run.code, 2, "{words:?}: {}", run.err);
        assert_eq!(run.out, "", "{words:?}: nothing on stdout");
        assert_eq!(run.err, format!("{sentence}\n{USAGE}\n"), "{words:?}: one sentence, then the usage, both on stderr");
        assert_eq!(snapshot(folder.path()), before, "{words:?}: no record was cancelled and nothing was made");
    }
}

#[test]
fn the_usage_line_lists_the_cancel_command() {
    let help = run_at(&["--help"], 0);
    assert_eq!((help.code, help.err.as_str()), (0, ""));
    assert!(help.out.contains("leko cancel FILE.zip --id ID [--write]"), "{}", help.out);
    assert!(USAGE.contains("leko cancel FILE.zip --id ID [--write]"));
    assert!(!USAGE.contains("leko cancel FILE.zip --id ID [--zone"), "a cancel has no --zone");
    // Every command the usage lists is still there.
    for command in ["leko session", "leko voice", "leko status", "leko add-block", "leko switch-off", "leko settle", "leko --help"] {
        assert!(USAGE.contains(command), "{command}");
    }
}
