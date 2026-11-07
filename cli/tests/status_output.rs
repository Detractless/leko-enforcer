//! `--json` makes the command write one line holding what the core decides for the same file, target,
//! moment and zone (AC-008): the same closed answer, the same closing and not-judged rules in the same
//! order, UTC times, `null` for what has no time. Without it only the sentence is written, and the two
//! always agree.

mod run_capture;
mod scratch_folder;
mod settings_builder;

use leko_core::config_time::zone_time::ZoneTime;
use leko_core::decision::block_cover::Target;
use leko_core::decision::decide_target::decide;
use leko_core::io::zip_check::{check_zip, ZipVerdict};
use leko_core::sessions::utc_moment::format_utc_moment;
use leko_core::status::next_flip::next_flip;
use run_capture::{json_of, moment, run_in_zone, status, status_at, Outcome};
use scratch_folder::Scratch;
use serde_json::{json, Value};
use settings_builder::{always, block, minutes, outside_hours, session, settings_bytes, write_settings};
use std::collections::HashSet;
use std::ffi::OsString;

const NOW: &str = "2026-11-05T15:00:00Z";

fn blocks() -> Vec<Value> {
    vec![
        block("alpha", json!({"apps": ["com.a"]}), json!([always("a1", &[]), minutes("m1")])),
        block("beta", json!({"apps": ["com.a"], "sites": ["example.com"]}), json!([session("s1", "2026-11-05T20:00:00Z")])),
        block("hours", json!({"apps": ["com.h"]}), json!([outside_hours("r", &[], &[("09:00", "17:00")])])),
    ]
}

/// What the core says about the target, written the way the command promises to write it.
fn from_the_core(blocks: &[Value], kind: &str, name: &str, zone_name: &str, now: i64) -> Value {
    let bytes = settings_bytes(blocks);
    let set = match check_zip(&bytes, &HashSet::new()) {
        ZipVerdict::Read(read) => read.set,
        ZipVerdict::Refused(refusal) => panic!("the test file is refused: {}", refusal.sentence),
    };
    let zone = ZoneTime::new(zone_name).expect("a known zone");
    let target = if kind == "app" { Target::App(name) } else { Target::Host(name) };
    let never_blocked = HashSet::new();
    let decision = decide(&set, target, now, &zone, &never_blocked);
    let until = next_flip(&set, target, &never_blocked, now, &zone).and_then(format_utc_moment);
    let rules = |list: &[leko_core::decision::decide_target::RuleRef]| -> Vec<Value> { list.iter().map(|r| json!({"block": r.block_id, "rule": r.rule_id, "kind": r.kind})).collect() };
    json!({
        "target": {"kind": kind, "name": name}, "at": format_utc_moment(now), "zone": zone_name,
        "closed": decision.closed, "closing": rules(&decision.closing), "not_judged": rules(&decision.not_judged), "until": until, "waiting": [],
    })
}

fn json_run(file: &std::path::Path, flags: &[&str], now: &str) -> Value {
    let run = status(file, flags, now, None);
    assert_eq!((run.code, run.err.as_str()), (0, ""), "{flags:?}: {}", run.err);
    json_of(&run)
}

fn sentence_run(file: &std::path::Path, flags: &[&str], now: &str) -> String {
    let run = status(file, flags, now, None);
    assert_eq!((run.code, run.err.as_str()), (0, ""), "{flags:?}: {}", run.err);
    run.out
}

#[test]
fn json_is_one_line_holding_what_the_core_decides_for_the_same_inputs() {
    let folder = Scratch::new("output");
    let file = write_settings(folder.path(), "settings.zip", &blocks());
    let ny = ["--zone", "America/New_York"];

    // The whole line, written out: closed by two rules, a third that counts use not judged, no end.
    let closed = json_run(&file, &["--json", "--app", "com.a", ny[0], ny[1]], NOW);
    let line = json!({
        "target": {"kind": "app", "name": "com.a"}, "at": "2026-11-05T15:00:00Z", "zone": "America/New_York", "closed": true,
        "closing": [{"block": "alpha", "rule": "a1", "kind": "always"}, {"block": "beta", "rule": "s1", "kind": "session"}],
        "not_judged": [{"block": "alpha", "rule": "m1", "kind": "minutes"}], "until": null, "waiting": [],
    });
    assert_eq!(closed, line);

    // Open with an end, and a zone that changes the answer: times are UTC either way, the zone is named.
    let open = json_run(&file, &["--app", "com.h", "--json", ny[0], ny[1]], NOW);
    assert_eq!((open["closed"].clone(), open["until"].clone(), open["zone"].clone()), (json!(false), json!("2026-11-05T22:00:00Z"), json!("America/New_York")));
    let tokyo = json_run(&file, &["--app", "com.h", "--zone", "Asia/Tokyo", "--json"], NOW);
    assert_eq!((tokyo["closed"].clone(), tokyo["until"].clone(), tokyo["at"].clone()), (json!(true), json!("2026-11-06T00:00:00Z"), json!("2026-11-05T15:00:00Z")));
    assert_eq!(tokyo["closing"], json!([{"block": "hours", "rule": "r", "kind": "outsideHours"}]));

    // The same as the core says, for apps and sites, covered and not, in three zones and at five moments.
    let blocks = blocks();
    for zone in ["America/New_York", "Asia/Tokyo", "UTC"] {
        for at in [NOW, "2026-11-05T23:30:00Z", "2026-11-05T20:00:00Z", "2026-11-05T19:59:59Z", "2026-03-08T06:59:00Z"] {
            for (kind, name) in [("app", "com.a"), ("app", "com.h"), ("app", "com.none"), ("site", "example.com"), ("site", "www.example.com"), ("site", "elsewhere.org")] {
                let flags = ["--json".to_string(), format!("--{}", if kind == "app" { "app" } else { "site" }), name.to_string(), "--zone".to_string(), zone.to_string()];
                let flags: Vec<&str> = flags.iter().map(String::as_str).collect();
                assert_eq!(json_run(&file, &flags, at), from_the_core(&blocks, kind, name, zone, moment(at)), "{kind} {name} {zone} {at}");
            }
        }
    }

    // `--json` can stand anywhere among the words; the file can stand anywhere too.
    let expected = from_the_core(&blocks, "app", "com.h", "Asia/Tokyo", moment(NOW));
    let path = file.to_str().expect("a text path").to_string();
    for words in [
        vec!["status", "--json", "--app", "com.h", "--zone", "Asia/Tokyo", &path],
        vec!["status", "--app", "com.h", "--zone", "Asia/Tokyo", &path, "--json"],
        vec!["status", &path, "--json", "--zone", "Asia/Tokyo", "--app", "com.h"],
        vec!["status", "--zone", "Asia/Tokyo", "--json", "--app", "com.h", &path],
    ] {
        let run = status_at(&words, moment(NOW), None);
        assert_eq!((run.code, run.err.as_str()), (0, ""), "{words:?}: {}", run.err);
        assert_eq!(json_of(&run), expected, "{words:?}");
    }
}

#[test]
fn the_sentence_and_the_json_agree_and_without_json_only_the_sentence_is_written() {
    let folder = Scratch::new("agree");
    let file = write_settings(folder.path(), "settings.zip", &blocks());
    for (flag, name) in [("--app", "com.a"), ("--app", "com.h"), ("--app", "com.none"), ("--site", "example.com"), ("--site", "elsewhere.org")] {
        for at in [NOW, "2026-11-05T23:30:00Z", "2026-11-06T02:00:00Z"] {
            let (sentence, line) = (sentence_run(&file, &[flag, name, "--zone", "America/New_York"], at), json_run(&file, &["--json", flag, name, "--zone", "America/New_York"], at));
            assert!(serde_json::from_str::<Value>(sentence.trim_end()).is_err(), "no JSON without --json: {sentence}");
            assert!(sentence.starts_with(if flag == "--app" { "App " } else { "Site " }) && sentence.ends_with("Nothing was saved or changed.\n"), "{sentence}");
            let state = if line["closed"] == json!(true) { " is closed " } else { " is open " };
            assert!(sentence.contains(state) && !sentence.contains(if state == " is open " { " is closed " } else { " is open " }), "{name} {at}: {sentence}");
            for list in ["closing", "not_judged"] {
                for rule in line[list].as_array().expect("a list") {
                    let named = format!("block \"{}\" rule \"{}\" ({})", rule["block"].as_str().unwrap(), rule["rule"].as_str().unwrap(), rule["kind"].as_str().unwrap());
                    assert!(sentence.contains(&named), "{named} is in {sentence}");
                }
            }
            let rules = line["closing"].as_array().unwrap().len() + line["not_judged"].as_array().unwrap().len();
            assert_eq!(sentence.matches("block \"").count(), rules, "{sentence}");
            assert_eq!(sentence.contains(" until "), line["until"].is_string(), "{sentence}");
        }
    }
}

#[test]
fn json_carries_the_whole_name_on_one_line_and_writes_null_for_a_moment_it_cannot_write() {
    let folder = Scratch::new("names");
    let file = write_settings(folder.path(), "settings.zip", &blocks());
    let long = "x".repeat(200);
    for name in ["a\nb\u{1b}[2J\"\\\u{7f}\t", "é日本語🙂", long.as_str()] {
        let value = json_run(&file, &["--json", "--app", name, "--zone", "UTC"], NOW);
        assert_eq!(value["target"], json!({"kind": "app", "name": name}), "{name:?}");
        let sentence = sentence_run(&file, &["--app", name, "--zone", "UTC"], NOW);
        assert_eq!(sentence.matches('\n').count(), 1, "one line: {sentence:?}");
        assert!(!sentence.contains('\u{1b}') && !sentence.contains('\u{7f}'), "{sentence:?}");
    }
    assert!(sentence_run(&file, &["--app", &long, "--zone", "UTC"], NOW).contains("..."), "the sentence is cut short, the JSON is not");

    // Year 10000 is a moment the text form cannot write: the line says null, not a made-up date.
    let beyond: i64 = 253_402_300_800_000;
    let args: Vec<OsString> = ["status", "--json", "--app", "com.h", "--zone", "UTC"].iter().map(OsString::from).chain([file.as_os_str().to_os_string()]).collect();
    let run: Outcome = run_in_zone(args, beyond, None);
    assert_eq!((run.code, run.err.as_str()), (0, ""), "{}", run.err);
    let value = json_of(&run);
    assert_eq!((value["at"].clone(), value["closed"].is_boolean()), (Value::Null, true));
}
