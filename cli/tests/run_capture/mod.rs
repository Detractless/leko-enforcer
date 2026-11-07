//! Runs the `leko` command line in this process with a moment the test chooses, and keeps what it
//! wrote to each stream. The real program reads the clock in `main.rs`; here the moment is fixed.
#![allow(dead_code)]

use leko_cli::run;
use leko_core::sessions::utc_moment::parse_utc_moment;
use serde_json::Value;
use std::ffi::OsString;
use std::path::Path;

pub const MINUTE_MS: i64 = 60_000;

/// What one run returned: the exit code and the two streams, as text.
pub struct Outcome {
    pub code: i32,
    pub out: String,
    pub err: String,
}

pub fn run_os(args: Vec<OsString>, now_ms: i64) -> Outcome {
    run_in_zone(args, now_ms, None)
}

/// A run on a computer whose time zone is `zone` (none: the computer's zone could not be found).
pub fn run_in_zone(args: Vec<OsString>, now_ms: i64, zone: Option<&str>) -> Outcome {
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = run(&args, now_ms, zone, &mut out, &mut err);
    Outcome { code, out: String::from_utf8(out).expect("stdout is text"), err: String::from_utf8(err).expect("stderr is text") }
}

pub fn run_at(args: &[&str], now_ms: i64) -> Outcome {
    run_os(args.iter().map(OsString::from).collect(), now_ms)
}

/// `run_in_zone` for words that are text.
pub fn status_at(args: &[&str], now_ms: i64, zone: Option<&str>) -> Outcome {
    run_in_zone(args.iter().map(OsString::from).collect(), now_ms, zone)
}

/// `leko status FLAGS... FILE` at the UTC moment `now` on a computer whose zone is `zone`.
pub fn status(file: &Path, flags: &[&str], now: &str, zone: Option<&str>) -> Outcome {
    let mut args: Vec<OsString> = vec!["status".into()];
    args.extend(flags.iter().map(OsString::from));
    args.push(file.as_os_str().to_os_string());
    run_in_zone(args, moment(now), zone)
}

pub fn moment(text: &str) -> i64 {
    parse_utc_moment(text).unwrap_or_else(|| panic!("{text} is not a moment"))
}

/// The end of a session of `minutes` that starts at `now_ms`: a part of a second rounds up.
pub fn expected_end(now_ms: i64, minutes: i64) -> i64 {
    (now_ms + minutes * MINUTE_MS + 999).div_euclid(1000) * 1000
}

/// The one JSON line a `--json` run wrote, as a value.
pub fn json_of(outcome: &Outcome) -> Value {
    assert_eq!(outcome.out.matches('\n').count(), 1, "one line: {:?}", outcome.out);
    serde_json::from_str(outcome.out.trim_end()).unwrap_or_else(|problem| panic!("not JSON ({problem}): {:?}", outcome.out))
}

/// The end the rule of a `--json` run names, in milliseconds.
pub fn ends_in_json(outcome: &Outcome) -> i64 {
    let json = json_of(outcome);
    moment(json["rule"]["endsAt"].as_str().expect("the rule has endsAt"))
}
