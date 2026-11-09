//! The waiting file: what is written reads back the same, and anything newer, odd or broken is
//! refused whole with one reason (AC-011).

use leko_core::delayed_changes::waiting_change::{ChangeKind, WaitingChange};
use leko_core::delayed_changes::waiting_file::{read_waiting, write_waiting, MAX_CHANGES};

const REQUESTED: i64 = 1_793_000_000_000;
const DUE: i64 = REQUESTED + 86_400_000;

fn change(block: &str) -> WaitingChange {
    WaitingChange::new(ChangeKind::SwitchOff, block, REQUESTED, DUE).expect("a good change")
}

fn record(block: &str) -> String {
    format!(r#"{{"kind":"switch-off","block":"{block}","requestedAt":"2026-10-26T07:33:20Z","dueAt":"2026-10-27T07:33:20Z"}}"#)
}

fn refused(text: &str) -> String {
    read_waiting(text).expect_err(&format!("{text:?} should be refused"))
}

#[test]
fn a_waiting_file_reads_back_the_same_and_a_newer_or_odd_one_is_refused() {
    // Written text is pretty JSON with the format number, the moments in UTC, and a final newline.
    let changes = vec![change("a"), change("b-2")];
    let text = write_waiting(&changes).expect("written");
    assert!(text.ends_with("}\n"), "{text:?}");
    assert!(text.contains("\"format\": 1"), "{text}");
    assert!(text.contains("\"requestedAt\": \"2026-10-26T07:33:20Z\""), "{text}");
    assert_eq!(read_waiting(&text).expect("read back"), changes);
    // An empty list is a good file, and writing it reads back empty.
    assert_eq!(read_waiting(&write_waiting(&[]).expect("written")).expect("read"), Vec::<WaitingChange>::new());
    // Writing twice gives the same text.
    assert_eq!(write_waiting(&changes).expect("again"), text);

    // Empty or broken text.
    assert_eq!(refused(""), "is empty");
    assert_eq!(refused(" \n\t"), "is empty");
    assert_eq!(refused("{"), "is not valid JSON");
    assert_eq!(refused("[]"), "must be a JSON object");
    assert_eq!(refused("null"), "must be a JSON object");
    // The format number: missing, wrong kind, zero, or newer.
    assert_eq!(refused(r#"{"changes":[]}"#), "needs \"format\": 1, 2 or 3");
    assert_eq!(refused(r#"{"format":"1","changes":[]}"#), "needs \"format\": 1, 2 or 3");
    assert_eq!(refused(r#"{"format":0,"changes":[]}"#), "needs \"format\": 1, 2 or 3");
    assert_eq!(refused(r#"{"format":1.5,"changes":[]}"#), "needs \"format\": 1, 2 or 3");
    assert_eq!(refused(r#"{"format":4,"changes":[]}"#), "is format 4, which is newer than the format 3 this program reads");
    // The list: missing, not a list, or too long.
    assert_eq!(refused(r#"{"format":1}"#), "needs \"changes\" as a list");
    assert_eq!(refused(r#"{"format":1,"changes":{}}"#), "needs \"changes\" as a list");
    let many = vec![record("a"); MAX_CHANGES + 1].join(",");
    assert_eq!(refused(&format!(r#"{{"format":1,"changes":[{many}]}}"#)), "holds more than 1000 changes");
    // A field this file does not have, at the top or in a record.
    assert_eq!(refused(r#"{"format":1,"changes":[],"note":"x"}"#), "has a field \"note\" that this file does not have");
    let odd = record("a").replace("\"kind\"", "\"extra\":1,\"kind\"");
    assert_eq!(refused(&format!(r#"{{"format":1,"changes":[{odd}]}}"#)), "change 1 has a field \"extra\" that this file does not have");
    // A record with a field missing, of the wrong kind, or not an object.
    let no_due = r#"{"kind":"switch-off","block":"a","requestedAt":"2026-10-26T07:33:20Z"}"#;
    assert_eq!(refused(&format!(r#"{{"format":1,"changes":[{no_due}]}}"#)), "change 1 needs \"dueAt\" as text");
    let number_block = record("a").replace("\"a\"", "5");
    assert_eq!(refused(&format!(r#"{{"format":1,"changes":[{number_block}]}}"#)), "change 1 needs \"block\" as text");
    assert_eq!(refused(r#"{"format":1,"changes":[3]}"#), "change 1 must be a JSON object");
    // A kind this program does not know.
    let unknown = record("a").replace("switch-off", "delete");
    assert_eq!(refused(&format!(r#"{{"format":1,"changes":[{unknown}]}}"#)), "change 1 has a kind \"delete\" that this program does not know");
    // A moment that is not a UTC moment, and a due moment before the request.
    let bad_moment = record("a").replace("2026-10-26T07:33:20Z", "tomorrow");
    assert!(refused(&format!(r#"{{"format":1,"changes":[{bad_moment}]}}"#)).starts_with("change 1: requestedAt must be a UTC moment"));
    let early = record("a").replace("2026-10-27T07:33:20Z", "2026-10-25T07:33:20Z");
    assert_eq!(refused(&format!(r#"{{"format":1,"changes":[{early}]}}"#)), "change 1: dueAt is before requestedAt");
    // A block id a settings file could not hold, and the same block parked twice.
    let bad_id = record("a/../b");
    assert!(refused(&format!(r#"{{"format":1,"changes":[{bad_id}]}}"#)).starts_with("change 1: the block id"));
    assert_eq!(refused(&format!(r#"{{"format":1,"changes":[{},{}]}}"#, record("a"), record("a"))), "parks the block \"a\" twice");
    // The second record's number is the one reported.
    let second_bad = format!(r#"{{"format":1,"changes":[{},{}]}}"#, record("a"), record("b").replace("dueAt", "due"));
    assert_eq!(refused(&second_bad), "change 2 has a field \"due\" that this file does not have");

    // A change the type itself refuses never gets into a file.
    assert!(WaitingChange::new(ChangeKind::SwitchOff, "a", REQUESTED + 1, DUE).is_err(), "not a whole second");
    assert!(WaitingChange::new(ChangeKind::SwitchOff, "a", REQUESTED, REQUESTED - 1000).is_err(), "due before request");
    assert!(WaitingChange::new(ChangeKind::SwitchOff, "a b", REQUESTED, DUE).is_err(), "bad id");
    assert!(WaitingChange::new(ChangeKind::SwitchOff, "a", REQUESTED, 300_000_000_000_000).is_err(), "past year 9999");
    let unwritable = WaitingChange { kind: ChangeKind::SwitchOff, block: "a".into(), requested_at: REQUESTED, due_at: 300_000_000_000_000, operations: vec![], rule: None, before: None, after: None };
    assert!(write_waiting(&[unwritable]).is_err());
}
