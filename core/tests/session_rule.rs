//! A session rule closes a block until an exact UTC moment, then counts as no rule at all. It is read
//! only when written `{"kind": "session", "endsAt": "YYYY-MM-DDTHH:MM:SSZ"}` with a real date and no
//! weekdays (AC-005), and it is judged by when it ends, as of the moment of the change (AC-006).

mod extension_cases;
mod vector_files;
mod zip_writer;

use extension_cases::{diff_problems, instant as utc_ms, load};
use leko_core::io::zip_check::{check_zip, Refusal, ZipVerdict};
use leko_core::models::specs::Rule;
use leko_core::sessions::session_compare::{session_ended, session_loosens};
use serde_json::{json, Value};
use std::collections::HashSet;
use zip_writer::zip_of;

const DIFF_CASES: usize = 22;
const END: &str = "2026-11-05T18:30:00Z";

fn session(id: &str, ends_at: &str) -> Value {
    json!({"id": id, "kind": "session", "endsAt": ends_at})
}

fn minutes_rule(id: &str) -> Value {
    json!({"id": id, "kind": "minutes", "amount": 30, "per": "day", "scope": "block"})
}

/// A block file, with `more` (door, pass) written into it.
fn block_file(id: &str, rules: &Value, more: &Value) -> String {
    let mut file = json!({"schema": 2, "id": id, "name": "B", "covers": {"apps": ["com.b"]}, "rules": rules});
    for (name, value) in more.as_object().into_iter().flatten() {
        file[name] = value.clone();
    }
    file.to_string()
}

fn verdict_of(rules: &Value, more: &Value) -> ZipVerdict {
    let good = block_file("a", &json!([minutes_rule("m")]), &json!({}));
    let under_test = block_file("b", rules, more);
    check_zip(&zip_of(&[("GUIDE.md", "guide"), ("blocks/a.json", &good), ("blocks/b.json", &under_test)]), &HashSet::new())
}

fn rules_read(rules: &Value, more: &Value) -> Vec<Rule> {
    match verdict_of(rules, more) {
        ZipVerdict::Read(read) => read.set.blocks.get("b").unwrap_or_else(|| panic!("block b is missing")).rules.clone(),
        ZipVerdict::Refused(refusal) => panic!("expected {rules} to be read, but it was refused: {}", refusal.sentence),
    }
}

fn refusal_of(rules: &Value, more: &Value) -> Refusal {
    match verdict_of(rules, more) {
        ZipVerdict::Refused(refusal) => refusal,
        ZipVerdict::Read(_) => panic!("expected a refusal for {rules}, but the zip was read"),
    }
}

fn many(count: usize) -> Vec<Value> {
    (0..count).map(|number| minutes_rule(&format!("r{number}"))).collect()
}

#[test]
fn session_rules_are_read_and_bad_ones_refuse() {
    let none = json!({});

    // Good: alone, beside another rule, at the edges of the calendar, at the 16-rule limit.
    let alone = rules_read(&json!([session("s", END)]), &none);
    assert_eq!(alone, vec![Rule { id: "s".to_string(), kind: "session".to_string(), days: Vec::new(), allowed: Vec::new(), ends_at: Some(utc_ms(END)) }]);
    assert_eq!(rules_read(&json!([minutes_rule("m"), session("s", END)]), &none).len(), 2);
    for moment in ["2028-02-29T23:59:59Z", "2000-02-29T00:00:00Z", "2026-12-31T23:59:59Z", "2026-01-01T00:00:00Z", "9999-12-31T23:59:59Z", "0000-01-01T00:00:00Z"] {
        assert_eq!(rules_read(&json!([session("s", moment)]), &none).len(), 1, "{moment}");
    }
    let mut fifteen = many(15);
    fifteen.push(session("s", END));
    assert_eq!(rules_read(&json!(fifteen), &none).len(), 16, "a session counts as one of the 16");

    // A session beside a rule that closes by its door: the block takes a pass as before.
    let door = json!({"door": {"kind": "wait", "seconds": 30}, "pass": {"length": "minutes", "minutes": 10, "scope": "block", "perDay": 3, "risingCost": "none"}});
    assert_eq!(rules_read(&json!([{"id": "r", "kind": "always"}, session("s", END)]), &door).len(), 2);

    // Bad: each is refused whole, naming blocks/b.json and the field, and nothing is read.
    let mut sixteen = many(16);
    sixteen.push(session("s", END));
    let with_days = json!({"id": "s", "kind": "session", "endsAt": END, "days": ["mon"]});
    let with_amount = json!({"id": "s", "kind": "session", "endsAt": END, "amount": 5});
    let with_per = json!({"id": "s", "kind": "session", "endsAt": END, "per": "day", "scope": "block"});
    let ends_on_always = json!({"id": "s", "kind": "always", "endsAt": END});
    let bad: Vec<(&str, Value, &str)> = vec![
        ("an offset", json!([session("s", "2026-11-05T18:30:00+02:00")]), "endsAt"),
        ("a zero offset", json!([session("s", "2026-11-05T18:30:00+00:00")]), "endsAt"),
        ("no Z", json!([session("s", "2026-11-05T18:30:00")]), "endsAt"),
        ("a lower case z", json!([session("s", "2026-11-05T18:30:00z")]), "endsAt"),
        ("a lower case t", json!([session("s", "2026-11-05t18:30:00Z")]), "endsAt"),
        ("a space for the T", json!([session("s", "2026-11-05 18:30:00Z")]), "endsAt"),
        ("a part of a second", json!([session("s", "2026-11-05T18:30:00.5Z")]), "endsAt"),
        ("no seconds", json!([session("s", "2026-11-05T18:30Z")]), "endsAt"),
        ("a two-digit year", json!([session("s", "26-11-05T18:30:00Z")]), "endsAt"),
        ("a date only", json!([session("s", "2026-11-05")]), "endsAt"),
        ("February 30", json!([session("s", "2026-02-30T00:00:00Z")]), "endsAt"),
        ("February 29 of a year that is not a leap year", json!([session("s", "2026-02-29T00:00:00Z")]), "endsAt"),
        ("February 29 of 2100", json!([session("s", "2100-02-29T00:00:00Z")]), "endsAt"),
        ("April 31", json!([session("s", "2026-04-31T00:00:00Z")]), "endsAt"),
        ("June 31", json!([session("s", "2026-06-31T00:00:00Z")]), "endsAt"),
        ("September 31", json!([session("s", "2026-09-31T00:00:00Z")]), "endsAt"),
        ("November 31", json!([session("s", "2026-11-31T00:00:00Z")]), "endsAt"),
        ("month 13", json!([session("s", "2026-13-01T00:00:00Z")]), "endsAt"),
        ("month 0", json!([session("s", "2026-00-10T00:00:00Z")]), "endsAt"),
        ("day 0", json!([session("s", "2026-11-00T00:00:00Z")]), "endsAt"),
        ("hour 24", json!([session("s", "2026-11-05T24:00:00Z")]), "endsAt"),
        ("minute 60", json!([session("s", "2026-11-05T18:60:00Z")]), "endsAt"),
        ("second 60", json!([session("s", "2026-11-05T18:30:60Z")]), "endsAt"),
        ("a sign in a digit place", json!([session("s", "2026-11-05T18:30:+0Z")]), "endsAt"),
        ("an empty text", json!([session("s", "")]), "endsAt"),
        ("a number", json!([{"id": "s", "kind": "session", "endsAt": 1_793_000_000}]), "endsAt"),
        ("null", json!([{"id": "s", "kind": "session", "endsAt": null}]), "endsAt"),
        ("a list", json!([{"id": "s", "kind": "session", "endsAt": [END]}]), "endsAt"),
        ("no endsAt", json!([{"id": "s", "kind": "session"}]), "endsAt"),
        ("a days list", json!([with_days]), "days"),
        ("an amount", json!([with_amount]), "amount"),
        ("a period and scope", json!([with_per]), "per"),
        ("endsAt on a rule that is not a session", json!([ends_on_always]), "endsAt"),
        ("two session rules", json!([session("s", END), session("t", "2026-11-06T18:30:00Z")]), "rules"),
        ("two session rules among others", json!([minutes_rule("m"), session("s", END), session("t", END)]), "rules"),
        ("a session as the 17th rule", json!(sixteen), "rules"),
    ];
    for (label, rules, field) in &bad {
        let refusal = refusal_of(rules, &none);
        assert_eq!(refusal.entry.as_deref(), Some("blocks/b.json"), "{label}: the refusal names the file");
        assert!(refusal.sentence.starts_with("blocks/b.json: "), "{label}: {}", refusal.sentence);
        assert!(refusal.sentence.contains(field), "{label}: the sentence names {field}: {}", refusal.sentence);
        let marked: Vec<&str> = refusal.files.iter().filter(|file| file.problem.is_some()).map(|file| file.path.as_str()).collect();
        assert_eq!(marked, vec!["blocks/b.json"], "{label}: only the bad file carries the problem");
    }

    // A block whose only rule is a session takes no pass: the rule does not close it by its door.
    let refusal = refusal_of(&json!([session("s", END)]), &door);
    assert!(refusal.sentence.contains("pass"), "a pass beside a session alone: {}", refusal.sentence);
    let refusal = refusal_of(&json!([minutes_rule("m"), session("s", END)]), &door);
    assert!(refusal.sentence.contains("pass"), "a pass beside a session and a minutes rule: {}", refusal.sentence);
    // A door without a pass is no problem for a session alone, as for any rule that does not close by its door.
    assert_eq!(rules_read(&json!([session("s", END)]), &json!({"door": {"kind": "wait", "seconds": 30}})).len(), 1);

    // The block with a good first block and a bad second one is not accepted on its own.
    let named = refusal_of(&json!([minutes_rule("m"), {"id": "s", "kind": "session", "endsAt": "2026-02-30T00:00:00Z"}]), &none);
    assert!(named.sentence.contains("rules[1]"), "the second rule is the bad one: {}", named.sentence);
}

#[test]
fn session_changes_are_judged_by_when_they_end() {
    let file = load("session-cases.json");
    assert_eq!(file.table.of("block.rules"), "rules", "the rules are judged one by one, by id");
    assert_eq!(diff_problems(&file, DIFF_CASES), Vec::<String>::new());

    // The readers refuse a session whose end cannot be read, so these answers are for a caller that
    // hands over a file the readers never saw: the cautious one, and only a session ends.
    let good = session("s", END);
    let unreadable = session("s", "tomorrow");
    assert!(session_loosens(&good, &unreadable) && session_loosens(&unreadable, &good) && session_loosens(&unreadable, &unreadable));
    assert!(!session_loosens(&good, &good));
    let at = |text: &str| utc_ms(text);
    assert!(session_ended(&good, at("2026-11-05T18:30:00Z")) && !session_ended(&good, at("2026-11-05T18:29:59Z")));
    assert!(!session_ended(&unreadable, i64::MAX), "a session with no readable end never ends");
    let always_with_end = json!({"id": "s", "kind": "always", "endsAt": "2020-01-01T00:00:00Z"});
    assert!(!session_ended(&always_with_end, at("2026-11-05T18:30:00Z")), "only a session rule can end");
}
