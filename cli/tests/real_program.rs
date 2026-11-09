//! The real `leko` program, started as a process: it counts from the real clock (AC-010), saves and
//! changes nothing and says so, also on empty input (AC-008), and tells success, refusal and
//! not-understood apart by exit code and stream, even when an output is closed (AC-009).

mod run_capture;

use leko_cli::run;
use run_capture::{ends_in_json, json_of, moment, run_with_closed, Closed, Outcome, MINUTE_MS};
use std::ffi::OsString;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn program() -> Command {
    Command::new(env!("CARGO_BIN_EXE_leko"))
}

fn outcome_of(output: Output) -> Outcome {
    Outcome {
        code: output.status.code().expect("the program ended by itself"),
        out: String::from_utf8(output.stdout).expect("stdout is text"),
        err: String::from_utf8(output.stderr).expect("stderr is text"),
    }
}

fn leko(args: &[&str]) -> Outcome {
    outcome_of(program().args(args).stdin(Stdio::null()).output().expect("the program starts"))
}

fn clock_ms() -> i64 {
    i64::try_from(SystemTime::now().duration_since(UNIX_EPOCH).expect("a clock after 1970").as_millis()).expect("a moment that fits")
}

/// A folder that is removed, even when read-only, when the test ends.
struct Scratch(PathBuf);

impl Scratch {
    fn new(label: &str) -> Scratch {
        static COUNT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!("leko-cli-{label}-{}-{}", std::process::id(), COUNT.fetch_add(1, Ordering::SeqCst)));
        std::fs::create_dir_all(&path).expect("a scratch folder");
        Scratch(path)
    }

    fn names(&self) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(&self.0).expect("readable").map(|entry| entry.expect("an entry").file_name().to_string_lossy().into_owned()).collect();
        names.sort();
        names
    }

    #[cfg(unix)]
    fn set_mode(&self, mode: u32) {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&self.0, std::fs::Permissions::from_mode(mode)).expect("permissions");
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        #[cfg(unix)]
        self.set_mode(0o755);
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The program started in `work`, with its home, config, data and temporary folders in `home`.
fn leko_in(work: &Path, home: &Path, args: &[&str]) -> Outcome {
    let mut command = program();
    command.args(args).current_dir(work).stdin(Stdio::null());
    for variable in ["HOME", "XDG_CONFIG_HOME", "XDG_DATA_HOME", "XDG_CACHE_HOME", "TMPDIR", "APPDATA", "LOCALAPPDATA"] {
        command.env(variable, home);
    }
    outcome_of(command.output().expect("the program starts"))
}

#[test]
fn the_real_program_ends_a_session_at_the_real_clock_plus_the_length() {
    let cases: [(Vec<&str>, i64); 6] = [
        (vec!["session", "--json", "30"], 30),
        (vec!["session", "--json", "1"], 1),
        (vec!["session", "--json", "2h"], 120),
        (vec!["session", "--json", "7d"], 10_080),
        (vec!["voice", "--json", "start a 25 minute session"], 25),
        (vec!["session", "--id", "focus", "--json", "45m"], 45),
    ];
    let check = |args: &[&str], minutes: i64, zone: Option<&str>| {
        let mut command = program();
        command.args(args).stdin(Stdio::null());
        if let Some(zone) = zone {
            command.env("TZ", zone);
        }
        let before = clock_ms();
        let run = outcome_of(command.output().expect("the program starts"));
        let after = clock_ms();
        assert_eq!(run.code, 0, "{args:?}: {}", run.err);
        let ends = ends_in_json(&run);
        let (earliest, latest) = (before + minutes * MINUTE_MS, after + minutes * MINUTE_MS + 1000);
        assert!(ends >= earliest, "{args:?} in {zone:?}: ends {ends} is before the clock plus {minutes} minutes ({earliest})");
        assert!(ends < latest, "{args:?} in {zone:?}: ends {ends} is after the clock plus {minutes} minutes ({latest})");
        assert_eq!(json_of(&run)["rule"]["endsAt"], json_of(&run)["lock"]["at"], "the rule and the seal end together");
    };
    for (args, minutes) in &cases {
        check(args, *minutes, None);
    }

    // The zone the machine is set to changes nothing: the end is in UTC.
    for zone in ["UTC", "America/New_York", "Asia/Kolkata", "Pacific/Chatham", "Nonsense/Zone", ""] {
        check(&["session", "--json", "30"], 30, Some(zone));
    }

    // Two runs a little over a second apart end at different moments, the later one later.
    let first = ends_in_json(&leko(&["session", "--json", "30"]));
    std::thread::sleep(Duration::from_millis(1_100));
    let second = ends_in_json(&leko(&["session", "--json", "30"]));
    assert!(second > first, "the later run ends later ({first} then {second})");

    // The sentence form names the same kind of moment: in UTC, close to the real clock plus 30 minutes.
    let before = clock_ms();
    let sentence = leko(&["session", "30"]);
    let after = clock_ms();
    let at = sentence.out.find("ends at ").expect("it says when it ends") + "ends at ".len();
    let ends = moment(&sentence.out[at..at + 20]);
    assert!(ends >= before + 30 * MINUTE_MS && ends < after + 30 * MINUTE_MS + 1000, "{}", sentence.out);
}

#[test]
fn running_twice_or_on_empty_input_changes_nothing_and_says_so() {
    let (work, home) = (Scratch::new("work"), Scratch::new("home"));
    let places_are_empty = || assert!(work.names().is_empty() && home.names().is_empty(), "files appeared: {:?} {:?}", work.names(), home.names());

    // The same command twice: both are made, both say nothing was saved, and no file appears.
    for _ in 0..2 {
        let run = leko_in(&work.0, &home.0, &["session", "30"]);
        assert_eq!((run.code, run.err.as_str()), (0, ""), "{}", run.err);
        assert!(run.out.contains("Nothing was saved or changed"), "{}", run.out);
        places_are_empty();
        let by_voice = leko_in(&work.0, &home.0, &["voice", "--json", "start a 30 minute session"]);
        assert_eq!(by_voice.code, 0, "{}", by_voice.err);
        places_are_empty();
    }

    // Empty input makes nothing: not even a default session; it says so and writes nothing.
    for words in ["", "   ", "\n"] {
        let run = leko_in(&work.0, &home.0, &["voice", words]);
        assert_eq!(run.code, 2, "{words:?}");
        assert_eq!(run.out, "", "{words:?}: no default session is printed");
        assert!(run.err.contains("Nothing was made"), "{words:?}: {}", run.err);
        places_are_empty();
    }
    for args in [vec!["voice"], vec!["session"], vec![], vec!["session", ""]] {
        let run = leko_in(&work.0, &home.0, &args);
        assert_eq!((run.code, run.out.as_str()), (2, ""), "{args:?}");
        assert!(!run.err.is_empty(), "{args:?}: it says why");
        places_are_empty();
    }

    // Refused requests change nothing either.
    let refused = leko_in(&work.0, &home.0, &["session", "0"]);
    assert_eq!((refused.code, refused.out.as_str()), (1, ""));
    assert!(refused.err.contains("Nothing was made"), "{}", refused.err);
    places_are_empty();

    // With the folders read-only a run still works, which shows it does not write.
    #[cfg(unix)]
    {
        work.set_mode(0o555);
        home.set_mode(0o555);
        let run = leko_in(&work.0, &home.0, &["session", "--json", "30"]);
        assert_eq!((run.code, run.err.as_str()), (0, ""), "{}", run.err);
        let empty = leko_in(&work.0, &home.0, &["voice", ""]);
        assert_eq!(empty.code, 2);
        places_are_empty();
        work.set_mode(0o755);
        home.set_mode(0o755);
    }

    // What the program started in the same folder printed twice is the same text but for the end.
    let sentence = |_: u8| leko_in(&work.0, &home.0, &["session", "5"]).out;
    let (a, b) = (sentence(0), sentence(1));
    assert_eq!(a.split("ends at").next(), b.split("ends at").next());
    assert!(a.ends_with("add --json to see the rule and the seal.\n") && b.ends_with(a.rsplit_once("changed").expect("a tail").1), "{a:?} {b:?}");
}

/// Runs the program with one output pointing at a pipe that nothing reads.
fn leko_with_closed(args: &[&str], closed: Closed) -> Outcome {
    let mut command = program();
    command.args(args).stdin(Stdio::null());
    outcome_of(run_with_closed(command, closed))
}

/// An output that fails when written to and/or when flushed.
struct Broken {
    write_fails: bool,
    flush_fails: bool,
}

impl Write for Broken {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        if self.write_fails { Err(io::ErrorKind::BrokenPipe.into()) } else { Ok(buffer.len()) }
    }

    fn flush(&mut self) -> io::Result<()> {
        if self.flush_fails { Err(io::ErrorKind::BrokenPipe.into()) } else { Ok(()) }
    }
}

/// The exit code and what went to a readable stderr when stdout fails as `out` says.
fn run_into_broken(args: &[&str], out: &mut Broken) -> (i32, String) {
    let args: Vec<OsString> = args.iter().map(OsString::from).collect();
    let mut err = Vec::new();
    let code = run(&args, moment("2026-11-05T10:00:00Z"), None, out, &mut err);
    (code, String::from_utf8(err).expect("stderr is text"))
}

#[test]
fn exit_codes_and_streams_tell_success_refusal_and_usage_apart() {
    // 0: a session is made, or help is printed: the answer on stdout, stderr empty.
    for args in [vec!["session", "30"], vec!["session", "--json", "30"], vec!["voice", "a 30 minute session"], vec!["--help"], vec!["session", "-h"]] {
        let run = leko(&args);
        assert_eq!((run.code, run.err.as_str()), (0, ""), "{args:?}: {}", run.err);
        assert!(run.out.ends_with('\n') && !run.out.trim().is_empty(), "{args:?}: {:?}", run.out);
    }

    // 1: understood and refused by the core: its sentence on stderr, stdout empty.
    for args in [
        vec!["session", "0"],
        vec!["session", "10081"],
        vec!["session", "8d"],
        vec!["session", "--id", "a b", "30"],
        vec!["voice", "a 8 day session"],
        vec!["voice", "--id", "no/good", "a 30 minute session"],
    ] {
        let run = leko(&args);
        assert_eq!((run.code, run.out.as_str()), (1, ""), "{args:?}");
        assert!(run.err.contains("Nothing was made") && !run.err.contains("Usage:"), "{args:?}: {}", run.err);
    }

    // 2: not understood: a sentence and the usage on stderr (words: what was heard), stdout empty.
    for args in [vec![], vec!["session"], vec!["bogus"], vec!["session", "1.5h"], vec!["session", "--wat", "30"], vec!["voice", "cancel my 30 minute session"], vec!["voice", "a session"]] {
        let run = leko(&args);
        assert_eq!((run.code, run.out.as_str()), (2, ""), "{args:?}");
        assert!(run.err.contains("Usage: leko session") || run.err.contains("Heard:"), "{args:?}: {}", run.err);
    }
    let codes: std::collections::BTreeSet<i32> = [leko(&["session", "30"]).code, leko(&["session", "0"]).code, leko(&["bogus"]).code].into_iter().collect();
    assert_eq!(codes.len(), 3, "the three outcomes have three different codes");

    // Whatever the arguments, the program ends with 0, 1 or 2, never a crash (101) or a signal.
    let long = "x".repeat(60_000);
    let long_session = format!("start a {} minute session", "9".repeat(50_000));
    let odd: Vec<Vec<&str>> = vec![
        vec!["session", &long],
        vec!["voice", &long],
        vec!["voice", &long_session],
        vec!["session", "--id", &long, "30"],
        vec!["session", "\u{7f}"],
        vec!["voice", "\u{1b}[2J session"],
        vec!["voice", "session", "30", "minutes", "--id", "a", "--json", "--help"],
    ];
    for args in &odd {
        let run = leko(args);
        assert!(matches!(run.code, 0..=2), "{} arguments: exit {}", args.len(), run.code);
        assert!(!run.err.contains("panicked") && !run.err.contains('\u{1b}'), "{:?}", run.err.chars().take(200).collect::<String>());
    }
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        let output = program().arg("session").arg(std::ffi::OsStr::from_bytes(&[b'3', 0xFF])).stdin(Stdio::null()).output().expect("the program starts");
        let run = outcome_of(output);
        assert_eq!((run.code, run.out.as_str()), (2, ""), "{}", run.err);
        assert!(run.err.contains("not valid text"), "{}", run.err);
    }

    // stdout closed: nothing can be shown, so it is exit 1 and a sentence on stderr, never a panic.
    for args in [vec!["session", "30"], vec!["session", "--json", "30"], vec!["voice", "a 30 minute session"], vec!["--help"]] {
        let run = leko_with_closed(&args, Closed::Stdout);
        assert_eq!(run.code, 1, "{args:?}: {}", run.err);
        assert!(run.err.contains("could not be written") && !run.err.contains("panicked"), "{args:?}: {}", run.err);
    }
    // A refusal or a usage problem writes nothing to stdout, so a closed stdout does not change it.
    assert_eq!(leko_with_closed(&["session", "0"], Closed::Stdout).code, 1);
    assert_eq!(leko_with_closed(&["bogus"], Closed::Stdout).code, 2);
    // stderr closed: the code is the same and nothing panics.
    assert_eq!(leko_with_closed(&["session", "0"], Closed::Stderr).code, 1);
    assert_eq!(leko_with_closed(&["bogus"], Closed::Stderr).code, 2);
    let made = leko_with_closed(&["session", "30"], Closed::Stderr);
    assert_eq!((made.code, made.err.as_str()), (0, ""));
    assert!(made.out.contains("Nothing was saved or changed"), "{}", made.out);

    // The same in this process with outputs that fail when written or when flushed.
    let ways = [(true, false), (false, true), (true, true)];
    for (write_fails, flush_fails) in ways {
        for args in [vec!["session", "30"], vec!["session", "--json", "30"], vec!["voice", "a 30 minute session"], vec!["--help"]] {
            let (code, err) = run_into_broken(&args, &mut Broken { write_fails, flush_fails });
            assert_eq!(code, 1, "{args:?} with write_fails={write_fails} flush_fails={flush_fails}");
            assert!(err.contains("could not be written"), "{args:?}: {err}");
        }
        assert_eq!(run_into_broken(&["session", "0"], &mut Broken { write_fails, flush_fails }).0, 1);
        assert_eq!(run_into_broken(&["bogus"], &mut Broken { write_fails, flush_fails }).0, 2);
    }
    let (code, err) = run_into_broken(&["session", "30"], &mut Broken { write_fails: false, flush_fails: false });
    assert_eq!((code, err.as_str()), (0, ""), "an output that works is not caught by these checks");
}
