//! Wrong or missing arguments make nothing: a usage line and one plain sentence on stderr, nothing on
//! stdout, exit 2 (AC-003); `--help` prints the usage on stdout and exits 0. The rule is named
//! `session` unless `--id` gives a name, and a bad name is refused with exit 1 (AC-004).

mod run_capture;

use leko_cli::session::session_arguments::USAGE;
use leko_core::config::read_block::ID_PROBLEM;
use run_capture::{json_of, moment, run_at, run_os};
use std::ffi::OsString;

const NOW: &str = "2026-11-05T10:00:00Z";

#[test]
fn wrong_or_missing_arguments_print_usage_on_stderr_and_exit_2() {
    let now = moment(NOW);
    let long_flag = "--".to_string() + &"x".repeat(2_000);
    let wrong: Vec<Vec<&str>> = vec![
        vec![],
        vec!["session"],
        vec!["voice"],
        vec!["voice", "--json"],
        vec!["voice", "--id", "x"],
        vec!["frobnicate"],
        vec!["SESSION", "30"],
        vec!["30"],
        vec!["session", "--wat", "30"],
        vec!["session", &long_flag, "30"],
        vec!["voice", "--wat", "start a 30 minute session"],
        vec!["voice", "start a 30 minute session", "-x"],
        vec!["session", "30", "45"],
        vec!["session", "30m", "x"],
        vec!["session", "30", "--json", "--json"],
        vec!["session", "--json", "30", "--json"],
        vec!["session", "--id"],
        vec!["session", "30", "--id"],
        vec!["session", "--id", "--json", "30"],
        vec!["session", "--id", "a", "--id", "b", "30"],
        vec!["session", "1.5h"],
        vec!["session", "1h30m"],
        vec!["session", "-5"],
        vec!["session", "+5"],
        vec!["session", "2H"],
        vec!["session", "٣٠"],
        vec!["session", "２０"],
        vec!["session", ""],
        vec!["session", " "],
        vec!["session", "five"],
    ];
    for args in &wrong {
        let run = run_at(args, now);
        assert_eq!(run.code, 2, "{args:?}: {}", run.err);
        assert_eq!(run.out, "", "{args:?}: nothing on stdout");
        assert!(run.err.contains("Usage: leko session"), "{args:?}: the usage line: {}", run.err);
        let first = run.err.lines().next().unwrap_or("");
        assert!(!first.is_empty() && !first.starts_with("Usage:"), "{args:?}: a sentence comes before the usage: {}", run.err);
        // The usage line grows with every new command, so the bound is the usage plus a short sentence.
        assert!(run.err.len() < USAGE.len() + 400, "{args:?}: a long argument is not echoed whole ({} bytes)", run.err.len());
    }

    // An argument that is not valid text is refused, not a crash (a lone 0xFF is not UTF-8).
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        let bad = OsString::from_vec(vec![b'3', 0xFF, b'0']);
        let run = run_os(vec![OsString::from("session"), bad.clone()], now);
        assert_eq!(run.code, 2);
        assert_eq!(run.out, "");
        assert!(run.err.contains("not valid text") && run.err.contains("Usage: leko session"), "{}", run.err);
        let run = run_os(vec![OsString::from("voice"), bad, OsString::from("session")], now);
        assert_eq!(run.code, 2, "a bad word among good ones");
    }

    // A good command line is not caught by these checks.
    assert_eq!(run_at(&["session", "30"], now).code, 0);
    assert_eq!(run_at(&["session", "--json", "--id", "focus", "30"], now).code, 0);
    assert_eq!(run_at(&["voice", "a", "25", "minute", "session"], now).code, 0, "words may arrive as several arguments");

    // Help goes to stdout and is a success.
    for args in [vec!["--help"], vec!["-h"], vec!["session", "--help"], vec!["voice", "a", "--help"], vec!["session", "30", "-h"]] {
        let run = run_at(&args, now);
        assert_eq!(run.code, 0, "{args:?}");
        assert_eq!(run.err, "", "{args:?}: nothing on stderr");
        assert!(run.out.contains("Usage: leko session") && run.out.contains("leko voice"), "{args:?}: {}", run.out);
    }
}

#[test]
fn rule_id_defaults_to_session_and_bad_ids_are_refused() {
    let now = moment(NOW);
    let rule_id = |args: &[&str]| {
        let run = run_at(args, now);
        assert_eq!(run.code, 0, "{args:?}: {}", run.err);
        json_of(&run)["rule"]["id"].as_str().expect("a rule id").to_string()
    };

    assert_eq!(rule_id(&["session", "--json", "30"]), "session", "the default name");
    assert_eq!(rule_id(&["voice", "--json", "start a 30 minute session"]), "session", "the default name by voice");
    assert_eq!(rule_id(&["session", "--json", "--id", "focus", "30"]), "focus");
    assert_eq!(rule_id(&["session", "30", "--json", "--id", "focus"]), "focus", "flags may follow the length");
    assert_eq!(rule_id(&["voice", "--id", "evening-2", "--json", "a 30 minute session"]), "evening-2");
    let longest = "a".repeat(64);
    for id in ["a", "A1", "a-b_c", "_", "-", longest.as_str()] {
        if id.starts_with('-') {
            continue; // a name that starts with a dash reads as a flag
        }
        assert_eq!(rule_id(&["session", "--json", "--id", id, "30"]), id);
    }

    // The core's own sentence, nothing on stdout, exit 1: the request was understood but refused.
    let too_long = "a".repeat(65);
    for id in ["", "a b", "a/b", "a.b", "a\\b", "é", "a\nb", "../x", "a:b", too_long.as_str()] {
        let refused = run_at(&["session", "--id", id, "30"], now);
        assert_eq!(refused.code, 1, "{id:?}");
        assert_eq!(refused.out, "", "{id:?}: nothing on stdout");
        assert!(refused.err.contains(ID_PROBLEM), "{id:?}: {}", refused.err);
        assert!(refused.err.contains("Nothing was made"), "{id:?}: {}", refused.err);
        let by_voice = run_at(&["voice", "--id", id, "a 30 minute session"], now);
        assert_eq!((by_voice.code, by_voice.out.as_str()), (1, ""), "{id:?} by voice");
    }
}
