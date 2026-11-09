//! The real `leko` program, started as a process, answers `status` from the real clock in the
//! computer's own time zone (AC-011), and changes nothing, uses the streams and exit codes the way the
//! other commands do, and does not panic when an output is closed (AC-010).

mod run_capture;
mod scratch_folder;
mod settings_builder;

use leko_cli::run;
use leko_core::config_time::zone_time::ZoneTime;
use leko_core::sessions::utc_moment::{format_utc_moment, parse_utc_moment};
use run_capture::{json_of, run_with_closed, Closed, Outcome};
use scratch_folder::Scratch;
use serde_json::{json, Value};
use settings_builder::{always, block, session, write_settings};
use std::ffi::OsString;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

const HOUR_MS: i64 = 3_600_000;

fn outcome_of(output: Output) -> Outcome {
    Outcome {
        code: output.status.code().expect("the program ended by itself"),
        out: String::from_utf8(output.stdout).expect("stdout is text"),
        err: String::from_utf8(output.stderr).expect("stderr is text"),
    }
}

fn command(args: &[&str], tz: Option<&str>) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_leko"));
    command.args(args).stdin(Stdio::null());
    match tz {
        Some(value) => command.env("TZ", value),
        None => command.env_remove("TZ"),
    };
    command
}

/// The real program run with `TZ` set to `tz` (none: not set at all).
fn leko(args: &[&str], tz: Option<&str>) -> Outcome {
    outcome_of(command(args, tz).output().expect("the program starts"))
}

fn clock_ms() -> i64 {
    i64::try_from(SystemTime::now().duration_since(UNIX_EPOCH).expect("a clock after 1970").as_millis()).expect("a moment that fits")
}

fn text(path: &Path) -> &str {
    path.to_str().expect("a text path")
}

/// What `/etc/localtime` points to, found without the program's help: the zone name, or none.
fn zone_of_this_computer() -> Option<String> {
    let target = std::fs::read_link("/etc/localtime").ok()?;
    let target = target.to_str()?;
    let name = &target[target.rfind("zoneinfo/")? + "zoneinfo/".len()..];
    ZoneTime::new(name).ok().map(|_| name.to_string())
}

#[test]
fn the_real_program_judges_at_the_real_clock_in_the_computers_zone() {
    let folder = Scratch::new("real-clock");
    let start = clock_ms() / 1000 * 1000;
    let text_of = |ms: i64| format_utc_moment(ms).expect("a moment with text");
    let file = write_settings(folder.path(), "settings.zip", &[
        block("soon", json!({"apps": ["com.soon"]}), json!([session("x", &text_of(start + HOUR_MS))])),
        block("past", json!({"apps": ["com.past"]}), json!([session("x", &text_of(start - HOUR_MS))])),
        block("later", json!({"apps": ["com.later"]}), json!([session("x", &text_of(start + 240 * HOUR_MS))])),
        block("now", json!({"apps": ["com.now"]}), json!([always("a", &[])])),
    ]);
    let file = text(&file);
    let json_for = |app: &str, zone_flag: &[&str], tz: Option<&str>| {
        let mut args = vec!["status", "--json", "--app", app];
        args.extend_from_slice(zone_flag);
        args.push(file);
        leko(&args, tz)
    };

    // The moment judged is the real clock: a session that ends in an hour closes until then, one that
    // ended an hour ago does not, and the moment the line names is within a minute of the clock.
    let (before, run) = (clock_ms(), json_for("com.soon", &[], Some("UTC")));
    let after = clock_ms();
    assert_eq!((run.code, run.err.as_str()), (0, ""), "{}", run.err);
    let line = json_of(&run);
    let at = parse_utc_moment(line["at"].as_str().expect("a moment")).expect("a UTC moment");
    assert!(at > before - 60_000 && at <= after, "judged at {at}, the clock read {before} to {after}");
    assert_eq!((line["closed"].clone(), line["until"].clone()), (json!(true), json!(text_of(start + HOUR_MS))));
    assert_eq!(json_of(&json_for("com.past", &[], Some("UTC")))["closed"], json!(false));
    assert_eq!(json_of(&json_for("com.later", &[], Some("UTC")))["until"], Value::Null, "ten days ahead is past the look-ahead");
    assert_eq!(json_of(&json_for("com.now", &[], Some("UTC")))["closed"], json!(true));

    // The zone is the computer's, as the TZ variable says (a leading colon is allowed), and --zone
    // beats it; a TZ the program cannot use is not turned into UTC.
    let zone_of = |app: &str, flag: &[&str], tz: Option<&str>| json_of(&json_for(app, flag, tz))["zone"].clone();
    assert_eq!(zone_of("com.now", &[], Some("America/New_York")), json!("America/New_York"));
    assert_eq!(zone_of("com.now", &[], Some(":Asia/Tokyo")), json!("Asia/Tokyo"));
    assert_eq!(zone_of("com.now", &["--zone", "UTC"], Some("Asia/Tokyo")), json!("UTC"));
    assert_eq!(zone_of("com.now", &["--zone", "Asia/Kolkata"], None), json!("Asia/Kolkata"));
    for tz in ["Mars/Base", ":", "not a zone"] {
        let run = json_for("com.now", &[], Some(tz));
        assert_eq!((run.code, run.out.as_str()), (1, ""), "TZ={tz:?}");
        assert!(run.err.contains("--zone") && run.err.contains("nothing was judged") && run.err.matches('\n').count() == 1, "TZ={tz:?}: {}", run.err);
    }

    // With no usable TZ, the computer's own zone is what /etc/localtime points to, else there is no answer.
    for tz in [None, Some("")] {
        let run = json_for("com.now", &[], tz);
        match zone_of_this_computer() {
            Some(name) => {
                assert_eq!((run.code, run.err.as_str()), (0, ""), "TZ={tz:?}: {}", run.err);
                assert_eq!(json_of(&run)["zone"], json!(name), "TZ={tz:?}");
            }
            None => {
                assert_eq!((run.code, run.out.as_str()), (1, ""), "TZ={tz:?}");
                assert!(run.err.contains("--zone"), "{}", run.err);
            }
        }
    }
}

/// Every file and folder under `folder` with its bytes and last change, to compare before and after.
fn snapshot(folder: &Path) -> Vec<(PathBuf, Vec<u8>, SystemTime)> {
    let mut found = Vec::new();
    let mut pending = vec![folder.to_path_buf()];
    while let Some(path) = pending.pop() {
        let facts = std::fs::symlink_metadata(&path).expect("readable");
        let bytes = if facts.is_file() { std::fs::read(&path).expect("readable") } else { Vec::new() };
        if facts.is_dir() {
            pending.extend(std::fs::read_dir(&path).expect("readable").map(|entry| entry.expect("an entry").path()));
        }
        found.push((path, bytes, facts.modified().expect("a time")));
    }
    found.sort();
    found
}

/// Runs the program with one output pointing at a pipe that nothing reads.
fn leko_with_closed(args: &[&str], closed: Closed) -> Outcome {
    outcome_of(run_with_closed(command(args, Some("UTC")), closed))
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

fn run_into_broken(args: &[&str], out: &mut Broken) -> (i32, String) {
    let args: Vec<OsString> = args.iter().map(OsString::from).collect();
    let mut err = Vec::new();
    let code = run(&args, 1_000_000_000_000, Some("UTC"), out, &mut err);
    (code, String::from_utf8(err).expect("stderr is text"))
}

#[test]
fn status_changes_nothing_uses_the_streams_and_exit_codes_and_survives_a_closed_pipe() {
    let (settings, home, elsewhere) = (Scratch::new("real-settings"), Scratch::new("real-home"), Scratch::new("real-cwd"));
    let file = write_settings(settings.path(), "settings.zip", &[block("b", json!({"apps": ["com.a"]}), json!([always("a1", &[])]))]);
    let file = text(&file);
    let run_once = |args: &[&str]| outcome_of(command(args, Some("UTC")).env("HOME", home.path()).current_dir(elsewhere.path()).output().expect("the program starts"));
    let (answer_args, json_args) = (["status", "--app", "com.a", file], ["status", "--json", "--app", "com.a", file]);

    // Folders that cannot be written to are no obstacle, and nothing in them changes: not the settings
    // file, its folder, the home folder, nor the folder the program ran in.
    let watched = [settings.path(), home.path(), elsewhere.path()];
    let before: Vec<_> = watched.iter().map(|folder| snapshot(folder)).collect();
    #[cfg(unix)]
    for folder in [&settings, &home, &elsewhere] {
        folder.set_mode(0o555);
    }
    let (sentence, line, again) = (run_once(&answer_args), run_once(&json_args), run_once(&json_args));
    #[cfg(unix)]
    for folder in [&settings, &home, &elsewhere] {
        folder.set_mode(0o755);
    }
    let after: Vec<_> = watched.iter().map(|folder| snapshot(folder)).collect();
    assert_eq!(before, after, "status changed something on disk");
    assert_eq!(settings.names(), ["settings.zip"]);

    // Success: the answer on stdout, nothing on stderr, exit 0, and the same answer each time (the
    // moment judged is the only thing that differs).
    for run in [&sentence, &line, &again] {
        assert_eq!((run.code, run.err.as_str()), (0, ""), "{}", run.err);
        assert_eq!(run.out.matches('\n').count(), 1);
    }
    assert!(sentence.out.starts_with("App \"com.a\" is closed for at least the next 8 days") && sentence.out.ends_with("Nothing was saved or changed.\n"), "{}", sentence.out);
    let without_moment = |run: &Outcome| {
        let mut value = json_of(run);
        value.as_object_mut().expect("an object").remove("at");
        value
    };
    assert_eq!(without_moment(&line), without_moment(&again));

    // A refusal is exit 1 with one line on stderr and nothing on stdout; a request that is not
    // understood is exit 2 with the usage on stderr and nothing on stdout.
    let refused = run_once(&["status", "--app", "com.a", "/no/such/settings.zip"]);
    assert_eq!((refused.code, refused.out.as_str(), refused.err.matches('\n').count()), (1, "", 1), "{}", refused.err);
    assert!(refused.err.contains("was not found") && !refused.err.contains("leko status"), "{}", refused.err);
    let not_understood = run_once(&["status", "--app", "com.a"]);
    assert_eq!((not_understood.code, not_understood.out.as_str()), (2, ""));
    assert!(not_understood.err.starts_with("No settings file was given.\n") && not_understood.err.contains("leko status"), "{}", not_understood.err);

    // stdout closed: nothing can be shown, so it is exit 1 and a sentence on stderr, never a panic. A
    // refusal or a usage problem writes nothing to stdout, so a closed stdout does not change it.
    for args in [&answer_args[..], &json_args[..]] {
        let run = leko_with_closed(args, Closed::Stdout);
        assert_eq!(run.code, 1, "{args:?}: {}", run.err);
        assert!(run.err.contains("could not be written") && !run.err.contains("panicked"), "{args:?}: {}", run.err);
    }
    assert_eq!(leko_with_closed(&["status", "--app", "com.a", "/no/such/settings.zip"], Closed::Stdout).code, 1);
    assert_eq!(leko_with_closed(&["status", "--app", "com.a"], Closed::Stdout).code, 2);
    // stderr closed: the codes are the same and nothing panics; a success does not need stderr.
    assert_eq!(leko_with_closed(&["status", "--app", "com.a", "/no/such/settings.zip"], Closed::Stderr).code, 1);
    assert_eq!(leko_with_closed(&["status", "--app", "com.a"], Closed::Stderr).code, 2);
    let made = leko_with_closed(&answer_args, Closed::Stderr);
    assert_eq!((made.code, made.err.as_str()), (0, ""));
    assert!(made.out.contains("Nothing was saved or changed"), "{}", made.out);

    // The same in this process, with outputs that fail when written or when flushed.
    for (write_fails, flush_fails) in [(true, false), (false, true), (true, true)] {
        for args in [&answer_args[..], &json_args[..]] {
            let (code, err) = run_into_broken(args, &mut Broken { write_fails, flush_fails });
            assert_eq!(code, 1, "{args:?} with write_fails={write_fails} flush_fails={flush_fails}");
            assert!(err.contains("could not be written"), "{args:?}: {err}");
        }
        assert_eq!(run_into_broken(&["status", "--app", "com.a"], &mut Broken { write_fails, flush_fails }).0, 2);
    }
    let (code, err) = run_into_broken(&answer_args, &mut Broken { write_fails: false, flush_fails: false });
    assert_eq!((code, err.as_str()), (0, ""), "an output that works is not caught by these checks");
}
