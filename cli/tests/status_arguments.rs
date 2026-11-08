//! Wrong or missing status arguments are not guessed at (AC-003): each prints one plain sentence and
//! the usage on stderr, prints nothing on stdout, reads no file (the file named here does not exist,
//! so a program that tried to read it would end with exit 1, not 2) and exits 2. `--help` prints the
//! usage, which names `status`, on stdout.

mod run_capture;

use leko_cli::session::session_arguments::USAGE;
use run_capture::{run_os, status_at};
use std::ffi::OsString;

const NOW: &str = "2026-11-05T15:00:00Z";

fn words(args: &[&str]) -> run_capture::Outcome {
    status_at(args, run_capture::moment(NOW), Some("America/New_York"))
}

#[test]
fn wrong_or_missing_status_arguments_print_usage_on_stderr_and_exit_2() {
    let flag_like = "--\u{1b}[2J";
    let cases: Vec<(Vec<&str>, &str)> = vec![
        (vec!["status"], "Say what to look at: --app NAME, --site HOST or --device."),
        (vec!["status", "f.zip"], "Say what to look at: --app NAME, --site HOST or --device."),
        (vec!["status", "--json"], "Say what to look at: --app NAME, --site HOST or --device."),
        (vec!["status", "--app", "x"], "No settings file was given."),
        (vec!["status", "--site", "x"], "No settings file was given."),
        (vec!["status", "a.zip", "b.zip", "--app", "x"], "Only one settings file can be given."),
        (vec!["status", "f.zip", "--app", "x", "--site", "y"], "Give either --app or --site, not both."),
        (vec!["status", "f.zip", "--site", "y", "--app", "x"], "Give either --app or --site, not both."),
        (vec!["status", "f.zip", "--app", ""], "--app needs an app name that is not empty."),
        (vec!["status", "f.zip", "--site", ""], "--site needs a website host that is not empty."),
        (vec!["status", "f.zip", "--app", "a", "--app", "b"], "--app was given twice."),
        (vec!["status", "f.zip", "--site", "a", "--site", "b"], "--site was given twice."),
        (vec!["status", "f.zip", "--app", "a", "--zone", "UTC", "--zone", "UTC"], "--zone was given twice."),
        (vec!["status", "f.zip", "--app", "a", "--json", "--json"], "--json was given twice."),
        (vec!["status", "f.zip", "--app"], "--app needs an app name after it."),
        (vec!["status", "f.zip", "--site"], "--site needs a website host after it."),
        (vec!["status", "f.zip", "--app", "a", "--zone"], "--zone needs a time zone name after it."),
        (vec!["status", "--app", "--json", "f.zip"], "--app needs an app name after it."),
        (vec!["status", "f.zip", "--app", "a", "--wat"], "\"--wat\" is not a flag of this command."),
        (vec!["status", "f.zip", "--app=a"], "\"--app=a\" is not a flag of this command."),
        (vec!["status", "f.zip", "--zone=UTC", "--app", "a"], "\"--zone=UTC\" is not a flag of this command."),
        (vec!["status", "f.zip", "--app", "a", "-x"], "\"-x\" is not a flag of this command."),
        (vec!["status", "f.zip", "--app", "a", "--"], "\"--\" is not a flag of this command."),
        (vec!["status", "f.zip", "--app", "a", flag_like], "\"--\\u{1b}[2J\" is not a flag of this command."),
    ];
    for (args, sentence) in &cases {
        let run = words(args);
        assert_eq!((run.code, run.out.as_str()), (2, ""), "{args:?}: {}", run.err);
        assert_eq!(run.err, format!("{sentence}\n{USAGE}\n"), "{args:?}");
        assert!(run.err.contains("leko status"), "the usage names the command: {args:?}");
        assert!(!run.err.contains('\u{1b}'), "{args:?}: a control character reached the terminal");
    }

    // A flag of 1 MB is cut short in the sentence, and the answer is the same: not understood.
    let long_flag = format!("--{}", "x".repeat(1_000_000));
    let run = words(&["status", "f.zip", "--app", "a", &long_flag]);
    assert_eq!((run.code, run.out.as_str()), (2, ""));
    assert!(run.err.len() < USAGE.len() + 200 && run.err.contains("..."), "{} bytes", run.err.len());

    // An argument that is not valid text is refused the same way (unix can pass any bytes).
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        for position in 0..3 {
            let mut args: Vec<OsString> = vec!["status".into(), "f.zip".into(), "--app".into()];
            args[position] = OsString::from_vec(vec![b'a', 0xFF]);
            args.push("x".into());
            let run = run_os(args, run_capture::moment(NOW));
            assert_eq!((run.code, run.out.as_str()), (2, ""), "{}", run.err);
            assert_eq!(run.err, "An argument is not valid text.\n".to_string() + USAGE + "\n");
        }
    }
}

#[test]
fn help_prints_the_usage_naming_status_on_stdout_wherever_it_is_given() {
    for args in [
        vec!["--help"],
        vec!["-h"],
        vec!["status", "--help"],
        vec!["status", "f.zip", "--app", "x", "-h"],
        vec!["status", "--app", "--help"],
        vec!["status", "--json", "--help", "f.zip"],
        vec!["session", "--help"],
    ] {
        let run = words(&args);
        assert_eq!((run.code, run.err.as_str()), (0, ""), "{args:?}: {}", run.err);
        assert_eq!(run.out, format!("{USAGE}\n"), "{args:?}");
        assert!(run.out.contains("leko status") && run.out.contains("--app NAME | --site HOST"), "{args:?}: {}", run.out);
    }
}
