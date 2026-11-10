//! Wrong or missing `add-block` arguments make nothing: one plain sentence and the usage on stderr,
//! nothing on stdout, no file read, exit 2 (AC-009); `--help` names `add-block` and exits 0. Every
//! bad line names a file that does not exist: had any been read, the run would exit 1 with a
//! "not found" sentence instead.

mod add_block_runs;
mod run_capture;
mod scratch_folder;
mod settings_builder;

use add_block_runs::{add, plain, APP, NOW};
use run_capture::{moment, run_at, run_os};
use leko_cli::session::session_arguments::USAGE;
use scratch_folder::Scratch;
use settings_builder::write_settings;
use std::ffi::OsString;

const MISSING: &str = "/no/such/file.zip";

/// `add-block MISSING` and then `flags`.
fn on_missing<'a>(flags: &[&'a str]) -> Vec<&'a str> {
    [&["add-block", MISSING][..], flags].concat()
}

#[test]
fn wrong_or_missing_add_block_arguments_print_usage_on_stderr_and_exit_2() {
    let now = moment(NOW);
    let long_flag = "--".to_string() + &"x".repeat(2_000);
    let long_value = "y".repeat(2_000);
    let wrong: Vec<Vec<&str>> = vec![
        // No command word, no file, two files.
        vec![],
        vec!["add-block"],
        vec!["add-block", "--name", "X", "--app", APP, "--always"],
        vec!["add-block", "--always"],
        on_missing(&["/no/such/other.zip", "--name", "X", "--app", APP, "--always"]),
        vec!["add-block", MISSING, "--name", "X", "--app", APP, "--always", "/no/such/other.zip"],
        // No name, no app and no site, no rule, two rules.
        on_missing(&["--app", APP, "--always"]),
        on_missing(&["--name", "X", "--always"]),
        on_missing(&["--name", "X", "--app", APP]),
        on_missing(&["--name", "X", "--app", APP, "--always", "--for", "30"]),
        on_missing(&["--name", "X", "--app", APP, "--always", "--open-hours", "09:00-17:00"]),
        on_missing(&["--name", "X", "--app", APP, "--open-hours", "09:00-17:00", "--for", "30m"]),
        // Days with a timed block, a bad day.
        on_missing(&["--name", "X", "--app", APP, "--for", "30", "--days", "mon"]),
        on_missing(&["--name", "X", "--app", APP, "--always", "--days", "funday"]),
        on_missing(&["--name", "X", "--app", APP, "--always", "--days", "mon,funday"]),
        on_missing(&["--name", "X", "--app", APP, "--always", "--days", "mon,mon"]),
        on_missing(&["--name", "X", "--app", APP, "--always", "--days", "mon,"]),
        on_missing(&["--name", "X", "--app", APP, "--always", "--days", ""]),
        // A bad HH:MM-HH:MM, a bad length.
        on_missing(&["--name", "X", "--app", APP, "--open-hours", "09:00"]),
        on_missing(&["--name", "X", "--app", APP, "--open-hours", "09:00-"]),
        on_missing(&["--name", "X", "--app", APP, "--open-hours", "ab:cd-ef:gh"]),
        on_missing(&["--name", "X", "--app", APP, "--open-hours", "09:60-10:00"]),
        on_missing(&["--name", "X", "--app", APP, "--open-hours", "09:00-25:00"]),
        on_missing(&["--name", "X", "--app", APP, "--open-hours", "09:00-10:00,"]),
        on_missing(&["--name", "X", "--app", APP, "--open-hours", ""]),
        on_missing(&["--name", "X", "--app", APP, "--for", "1.5h"]),
        on_missing(&["--name", "X", "--app", APP, "--for", "soon"]),
        on_missing(&["--name", "X", "--app", APP, "--for", ""]),
        on_missing(&["--name", "X", "--app", APP, "--always", "--wait", "soon"]),
        on_missing(&["--name", "X", "--app", APP, "--always", "--wait", "1.5"]),
        // A wait and a wall together; a thing named twice.
        on_missing(&["--name", "X", "--app", APP, "--always", "--wall", "--wait", "30"]),
        on_missing(&["--name", "X", "--app", APP, "--always", "--wait", "30", "--wall"]),
        on_missing(&["--name", "X", "--app", APP, "--app", APP, "--always"]),
        on_missing(&["--name", "X", "--site", "example.org", "--site", "example.org", "--always"]),
        on_missing(&["--name", "X", "--name", "Y", "--app", APP, "--always"]),
        on_missing(&["--name", "X", "--app", APP, "--always", "--message", "a", "--message", "b"]),
        on_missing(&["--name", "X", "--app", APP, "--always", "--days", "mon", "--days", "tue"]),
        on_missing(&["--name", "X", "--app", APP, "--for", "30", "--for", "40"]),
        on_missing(&["--name", "X", "--app", APP, "--always", "--wait", "5", "--wait", "6"]),
        on_missing(&["--name", "X", "--app", APP, "--open-hours", "09:00-10:00", "--open-hours", "11:00-12:00"]),
        on_missing(&["--name", "X", "--app", APP, "--always", "--always"]),
        on_missing(&["--name", "X", "--app", APP, "--always", "--wall", "--wall"]),
        on_missing(&["--name", "X", "--app", APP, "--always", "--write", "--write"]),
        // A flag missing its value, whether last or followed by another flag.
        on_missing(&["--app", APP, "--always", "--name"]),
        on_missing(&["--name", "--app", APP, "--always"]),
        on_missing(&["--name", "X", "--app", "--always"]),
        on_missing(&["--name", "X", "--always", "--app"]),
        on_missing(&["--name", "X", "--app", APP, "--always", "--site"]),
        on_missing(&["--name", "X", "--app", APP, "--for"]),
        on_missing(&["--name", "X", "--app", APP, "--open-hours"]),
        on_missing(&["--name", "X", "--app", APP, "--always", "--days"]),
        on_missing(&["--name", "X", "--app", APP, "--always", "--wait"]),
        on_missing(&["--name", "X", "--app", APP, "--always", "--message"]),
        on_missing(&["--name", "X", "--app", APP, "--always", "--message", "--write"]),
        // Unknown flags, and long ones that must not be echoed whole.
        on_missing(&["--name", "X", "--app", APP, "--always", "--wat"]),
        on_missing(&["--name", "X", "--app", APP, "--always", "-x"]),
        on_missing(&["--name", "X", "--app", APP, "--always", "--WRITE"]),
        on_missing(&["--name", "X", "--app", APP, "--always", "--writes"]),
        on_missing(&["--name", "X", "--app", APP, "--always", &long_flag]),
        on_missing(&["--name", "X", "--app", APP, "--always", "--wait", &long_value]),
        on_missing(&["--name", "X", "--app", APP, "--always", "--days", &long_value]),
        on_missing(&["--name", "X", "--app", APP, "--open-hours", &long_value]),
        on_missing(&["--name", "X", "--app", APP, "--for", &long_value]),
    ];
    for args in &wrong {
        let run = run_at(args, now);
        let shown: Vec<String> = args.iter().map(|word| word.chars().take(30).collect()).collect();
        assert_eq!(run.code, 2, "{shown:?}: {}", run.err);
        assert_eq!(run.out, "", "{shown:?}: nothing on stdout");
        assert!(run.err.contains("Usage: leko") && run.err.contains("leko add-block FILE.zip"), "{shown:?}: the usage: {}", run.err);
        let first = run.err.lines().next().unwrap_or("");
        assert!(!first.is_empty() && !first.starts_with("Usage:"), "{shown:?}: a sentence comes before the usage: {}", run.err);
        assert!(!run.err.contains("No such file") && !run.err.contains("not found"), "{shown:?}: no file was read: {}", run.err);
        assert!(run.err.len() < USAGE.len() + 200, "{shown:?}: a long argument is not echoed whole ({} bytes)", run.err.len());
    }

    // An argument that is not valid text is refused, not a crash (a lone 0xFF is not UTF-8).
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        let bad = || OsString::from_vec(vec![b'a', 0xFF, b'b']);
        let words = |list: &[&str]| list.iter().map(OsString::from).collect::<Vec<_>>();
        let good = words(&["--name", "X", "--app", APP, "--always"]);
        let cases: Vec<Vec<OsString>> = vec![
            [words(&["add-block"]), vec![bad()], good.clone()].concat(),
            [words(&["add-block", MISSING, "--name"]), vec![bad()], words(&["--app", APP, "--always"])].concat(),
            [words(&["add-block", MISSING, "--name", "X", "--app"]), vec![bad()], words(&["--always"])].concat(),
            [words(&["add-block", MISSING, "--name", "X", "--app", APP, "--always", "--message"]), vec![bad()]].concat(),
            [words(&["add-block", MISSING]), good.clone(), vec![bad()]].concat(),
            vec![bad()],
        ];
        for args in cases {
            let run = run_os(args, now);
            assert_eq!(run.code, 2, "{}", run.err);
            assert_eq!(run.out, "");
            assert!(run.err.contains("not valid text") && run.err.contains("Usage: leko"), "{}", run.err);
        }
    }

    // A good command line is not caught by these checks: it reaches the file and finds none.
    let run = run_at(&on_missing(&plain()), now);
    assert_eq!(run.code, 1, "a good line is understood and then fails on the missing file: {}", run.err);
    let folder = Scratch::new("arguments");
    let file = write_settings(folder.path(), "settings.zip", &[]);
    assert_eq!(add(&file, &plain()).code, 0);

    // Help names `add-block`, goes to stdout and is a success, wherever it stands.
    for args in [vec!["--help"], vec!["-h"], vec!["add-block", "--help"], vec!["add-block", "-h"], on_missing(&["--name", "X", "--help"]), on_missing(&["--wat", "-h"])] {
        let run = run_at(&args, now);
        assert_eq!(run.code, 0, "{args:?}: {}", run.err);
        assert!(run.out.contains("leko add-block FILE.zip --name NAME"), "{args:?}: {}", run.out);
        assert_eq!(run.err, "", "{args:?}");
    }
}
