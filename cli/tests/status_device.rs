//! `leko status FILE --device` says what the device blocks ask the computer to do at the moment it is
//! given: the strongest action, every rule that asks for it, and when the action next changes. Every
//! sentence is compared whole, and the JSON line is compared as a value.

mod run_capture;
mod scratch_folder;
mod settings_builder;

use run_capture::{json_of, status, Outcome};
use scratch_folder::Scratch;
use serde_json::{json, Value};
use settings_builder::{always, session, write_settings};

const NOON: &str = "2026-11-05T15:00:00Z";
const ZONE: [&str; 2] = ["--zone", "America/New_York"];
const TAIL: &str = "Nothing was saved or changed.\n";
const JUDGED: &str = "judged at 2026-11-05T10:00:00-05:00 in America/New_York.";

fn settings(blocks: &[Value]) -> (Scratch, std::path::PathBuf) {
    let folder = Scratch::new("status-device");
    let file = write_settings(folder.path(), "settings.zip", blocks);
    (folder, file)
}

fn device_block(id: &str, device: &str, rules: Value) -> Value {
    json!({"schema": 2, "id": id, "name": "B", "on": true, "device": device, "rules": rules, "door": {"kind": "wall"}})
}

fn ask(file: &std::path::Path, flags: &[&str], now: &str) -> Outcome {
    let flags = [flags, &ZONE[..]].concat();
    status(file, &flags, now, None)
}

fn answered(run: &Outcome) -> &str {
    assert_eq!((run.code, run.err.as_str()), (0, ""), "{}", run.err);
    assert!(run.out.ends_with('\n') && run.out.matches('\n').count() == 1, "one line: {:?}", run.out);
    &run.out
}

#[test]
fn the_strongest_closed_device_block_is_reported_with_its_rules() {
    let (_folder, file) = settings(&[
        device_block("b-lock", "lock", json!([always("a1", &[])])),
        device_block("a-logoff", "logoff", json!([always("l1", &[])])),
        device_block("c-shutdown", "shutdown", json!([session("s2", "2026-11-05T20:00:00Z")])),
    ]);
    let run = ask(&file, &["--device"], NOON);
    let closers = r#"Closed by block "a-logoff" rule "l1" (always); block "b-lock" rule "a1" (always); block "c-shutdown" rule "s2" (session)."#;
    assert_eq!(answered(&run), format!("Device: the computer is shut down until 2026-11-05T15:00:00-05:00, {JUDGED} {closers} {TAIL}"));

    // The action is named as the moment changes: the shutdown session has ended, and the logoff and
    // lock blocks still close the computer.
    let later = ask(&file, &["--device"], "2026-11-05T23:30:00Z");
    let judged_later = "judged at 2026-11-05T18:30:00-05:00 in America/New_York.";
    assert_eq!(answered(&later), format!("Device: the user is logged out for at least the next 8 days, {judged_later} Closed by block \"a-logoff\" rule \"l1\" (always); block \"b-lock\" rule \"a1\" (always). {TAIL}"));
}

#[test]
fn an_open_device_is_reported_open_and_a_switched_off_or_ended_block_does_nothing() {
    let (_folder, file) = settings(&[
        json!({"schema": 2, "id": "off", "name": "B", "on": false, "device": "lock", "rules": [always("o1", &[])], "door": {"kind": "wall"}}),
        device_block("ended", "lock", json!([session("e1", "2026-11-05T14:00:00Z")])),
    ]);
    let empty = ask(&file, &["--device"], NOON);
    assert_eq!(answered(&empty), format!("Device: no device block is closed, {JUDGED} For at least the next 8 days nothing will lock, log out or shut down. {TAIL}"));

    // No blocks at all is the same answer, and the same inputs give the same sentence.
    let (_other, none) = settings(&[]);
    assert_eq!(answered(&ask(&none, &["--device"], NOON)), answered(&empty));
}

#[test]
fn the_device_line_in_json_carries_the_action_and_the_closing_rules() {
    let (_folder, file) = settings(&[
        device_block("b-lock", "lock", json!([always("a1", &[])])),
        device_block("a-shutdown", "shutdown", json!([session("s1", "2026-11-05T20:00:00Z")])),
    ]);
    let run = ask(&file, &["--device", "--json"], NOON);
    assert_eq!((run.code, run.err.as_str()), (0, ""), "{}", run.err);
    assert_eq!(run.out.matches('\n').count(), 1, "one line: {:?}", run.out);
    let value: Value = json_of(&run);
    assert_eq!(value, json!({
        "target": {"kind": "device", "name": ""},
        "at": "2026-11-05T15:00:00Z",
        "zone": "America/New_York",
        "closed": true,
        "action": "shutdown",
        "closing": [
            {"block": "a-shutdown", "rule": "s1", "kind": "session", "device": "shutdown"},
            {"block": "b-lock", "rule": "a1", "kind": "always", "device": "lock"},
        ],
        "until": "2026-11-05T20:00:00Z",
    }), "{}", run.out);
}

#[test]
fn one_of_app_site_or_device_must_be_asked_for() {
    let (_folder, file) = settings(&[device_block("freeze", "lock", json!([always("a1", &[])]))]);
    for (flags, sentence) in [
        (vec!["--device", "--app", "com.a"], "Give either --app or --device, not both."),
        (vec!["--device", "--site", "example.com"], "Give either --site or --device, not both."),
        (vec!["--device", "--device"], "--device was given twice."),
    ] {
        let run = ask(&file, &flags, NOON);
        assert_eq!(run.code, 2, "{flags:?}: {}", run.err);
        assert!(run.out.is_empty(), "{flags:?}: nothing on stdout");
        assert!(run.err.starts_with(sentence), "{flags:?}: {}", run.err);
    }
    let run = ask(&file, &[], NOON);
    assert_eq!(run.code, 2, "{}", run.err);
    assert!(run.err.starts_with("Say what to look at: --app NAME, --site HOST or --device."), "{}", run.err);
}
