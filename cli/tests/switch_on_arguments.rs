//! Wrong or missing words after `leko switch-on` make nothing (AC-009): one plain sentence and the
//! usage line (which lists `leko switch-on`) on stderr, nothing on stdout, exit 2, and no file touched.
//! A missing `--id` never switches on the first block, and an unknown flag (including `--zone` and
//! `--json`, which a switch-on has no use for) is never ignored.

mod delay_runs;
#[path = "../../core/tests/delay_sets/mod.rs"]
mod delay_sets;
mod run_capture;
mod scratch_folder;
#[path = "../../core/tests/zip_writer/mod.rs"]
mod zip_writer;

use delay_runs::{snapshot, write_zip};
use delay_sets::off_block;
use leko_cli::session::session_arguments::USAGE;
use run_capture::run_at;
use scratch_folder::Scratch;

#[test]
fn bad_arguments_print_usage_and_exit_two() {
    let folder = Scratch::new("switch-on-arguments");
    let file = write_zip(folder.path(), "s.zip", &[off_block("a"), off_block("b")], &[]);
    let before = snapshot(folder.path());
    let f = file.to_str().expect("a text path");

    let flag = |name: &str| format!("\"{name}\" is not a flag of this command.");
    let missing = "Say which block to switch on: --id ID.".to_string();
    let cases: Vec<(Vec<&str>, String)> = vec![
        (vec!["switch-on"], "No settings file was given.".to_string()),
        (vec!["switch-on", "--id", "a"], "No settings file was given.".to_string()),
        (vec!["switch-on", "--write"], "No settings file was given.".to_string()),
        (vec!["switch-on", f], missing.clone()),
        (vec!["switch-on", f, "--write"], missing),
        (vec!["switch-on", f, "--id"], "--id needs a block id after it.".to_string()),
        (vec!["switch-on", f, "--id", "--write"], "--id needs a block id after it.".to_string()),
        (vec!["switch-on", f, "--id", ""], "--id needs a block id after it.".to_string()),
        (vec!["switch-on", f, "--id", "a", "--id", "b"], "--id was given twice.".to_string()),
        (vec!["switch-on", f, "--id", "a", "--id", "a", "--write"], "--id was given twice.".to_string()),
        (vec!["switch-on", f, "--id", "a", "--write", "--write"], "--write was given twice.".to_string()),
        (vec!["switch-on", f, "--id", "a", "--zone", "UTC"], flag("--zone")),
        (vec!["switch-on", f, "--id", "a", "--zone"], flag("--zone")),
        (vec!["switch-on", f, "--id", "a", "--json"], flag("--json")),
        (vec!["switch-on", f, "--id", "a", "--wat"], flag("--wat")),
        (vec!["switch-on", f, "--id", "a", "-x", "--write"], flag("-x")),
        (vec!["switch-on", f, "other.zip", "--id", "a"], "Only one settings file can be given.".to_string()),
        (vec!["switch-on", f, "--id", "a", "other.zip", "--write"], "Only one settings file can be given.".to_string()),
    ];
    for (words, sentence) in &cases {
        let run = run_at(words, 0);
        assert_eq!(run.code, 2, "{words:?}: {}", run.err);
        assert_eq!(run.out, "", "{words:?}: nothing on stdout");
        assert_eq!(run.err, format!("{sentence}\n{USAGE}\n"), "{words:?}: one sentence, then the usage, both on stderr");
        assert_eq!(snapshot(folder.path()), before, "{words:?}: no block was switched on and nothing was made");
    }
}

#[test]
fn the_usage_line_lists_the_switch_on_command() {
    let help = run_at(&["--help"], 0);
    assert_eq!((help.code, help.err.as_str()), (0, ""));
    assert!(help.out.contains("leko switch-on FILE.zip --id ID [--write]"), "{}", help.out);
    assert!(USAGE.contains("leko switch-on FILE.zip --id ID [--write]"));
    assert!(!USAGE.contains("leko switch-on FILE.zip --id ID [--zone"), "a switch-on has no --zone");
    // Every command the usage listed before is still there.
    for command in ["leko session", "leko voice", "leko status", "leko add-block", "leko switch-off", "leko settle", "leko cancel", "leko --help"] {
        assert!(USAGE.contains(command), "{command}");
    }
}
