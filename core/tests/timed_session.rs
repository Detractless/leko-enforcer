//! The session maker: from the current moment and a number of minutes (1 to 10080) it returns a
//! session rule and a moment seal that end at the same instant, exactly that many minutes later,
//! also across a clock change, and both read back through the readers.

mod extension_cases;
mod vector_files;
mod zip_writer;

use extension_cases::instant;
use leko_core::io::zip_check::{check_zip, ZipVerdict};
use leko_core::models::specs::Lock;
use leko_core::sessions::timed_session::{make_timed_session, TimedSession, MAX_SESSION_MINUTES};
use leko_core::sessions::utc_moment::parse_utc_moment;
use serde_json::json;
use std::collections::HashSet;
use zip_writer::zip_of;

const MINUTE_MS: i64 = 60_000;
const LAST_SECOND_OF_9999_MS: i64 = 253_402_300_799_000;

fn made(now_ms: i64, minutes: i64) -> TimedSession {
    make_timed_session(now_ms, minutes, "session").unwrap_or_else(|sentence| panic!("{minutes} minutes at {now_ms} was refused: {sentence}"))
}

/// The session's rule and seal in a block file, read through the zip reader.
fn read_back(session: &TimedSession) -> (Vec<(String, String)>, Lock) {
    let minutes_rule = json!({"id": "m", "kind": "minutes", "amount": 30, "per": "day", "scope": "block"});
    let block = json!({"schema": 2, "id": "b", "name": "B", "covers": {"apps": ["com.b"]}, "rules": [minutes_rule, session.rule], "lock": session.lock}).to_string();
    match check_zip(&zip_of(&[("GUIDE.md", "guide"), ("blocks/b.json", &block)]), &HashSet::new()) {
        ZipVerdict::Read(read) => {
            let block = read.set.blocks.get("b").expect("block b");
            (block.rules.iter().map(|rule| (rule.id.clone(), rule.kind.clone())).collect(), block.lock.clone())
        }
        ZipVerdict::Refused(refusal) => panic!("the session is refused when read back: {}", refusal.sentence),
    }
}

#[test]
fn rule_and_seal_end_at_the_same_moment() {
    // Whole-second starts: the end is exactly `minutes` minutes later, written and as an instant.
    let start = instant("2026-11-05T12:00:00Z");
    for minutes in [1, 2, 59, 60, 61, 120, 1440, 5000, MAX_SESSION_MINUTES] {
        let session = made(start, minutes);
        assert_eq!(session.ends_ms, start + minutes * MINUTE_MS, "{minutes} minutes");
        assert_eq!(parse_utc_moment(&session.ends_at), Some(session.ends_ms), "{minutes} minutes: the text names the instant");
        assert_eq!(session.rule, json!({"id": "session", "kind": "session", "endsAt": session.ends_at}), "{minutes} minutes: the rule");
        assert_eq!(session.lock, json!({"level": "sealedAt", "at": session.ends_at}), "{minutes} minutes: the seal");
        let (rules, lock) = read_back(&session);
        assert_eq!(rules, vec![("m".to_string(), "minutes".to_string()), ("session".to_string(), "session".to_string())], "{minutes} minutes: the rule reads back");
        assert_eq!(lock, Lock::SealedAt { at: session.ends_ms, key: None }, "{minutes} minutes: the seal reads back");
    }
    assert_eq!(made(start, 90).ends_at, "2026-11-05T13:30:00Z");
    assert_eq!(made(start, MAX_SESSION_MINUTES).ends_at, "2026-11-12T12:00:00Z");

    // Across a clock change the length is still elapsed time: a 2-hour session that starts at
    // 00:30 New York time on the night clocks go forward ends at 03:30, 7200000 ms later.
    let spring = instant("2027-03-14T00:30:00-05:00");
    let across = made(spring, 120);
    assert_eq!(across.ends_ms - spring, 7_200_000);
    assert_eq!(across.ends_ms, instant("2027-03-14T03:30:00-04:00"));
    assert_eq!(across.ends_at, "2027-03-14T07:30:00Z");
    let autumn = instant("2026-11-01T00:30:00-04:00");
    let long_night = made(autumn, 120);
    assert_eq!(long_night.ends_ms - autumn, 7_200_000);
    assert_eq!(long_night.ends_ms, instant("2026-11-01T01:30:00-05:00"));

    // A part of a second rounds up to the next whole second: never shorter than asked, under a second longer.
    for part in [1, 250, 500, 999] {
        let session = made(start + part, 5);
        let exact = start + part + 5 * MINUTE_MS;
        assert_eq!(session.ends_ms, start + 5 * MINUTE_MS + 1000, "a start {part} ms into a second");
        assert!(session.ends_ms >= exact && session.ends_ms - exact < 1000, "a start {part} ms into a second");
    }
    let before_epoch = made(-1500, 1);
    assert_eq!(before_epoch.ends_ms, 59_000);
    assert_eq!(before_epoch.ends_at, "1970-01-01T00:00:59Z");

    // Refused with a sentence: no minutes, a negative number, one over 7 days, and sums that do not fit.
    let refused: Vec<(&str, Result<TimedSession, String>)> = vec![
        ("0 minutes", make_timed_session(start, 0, "s")),
        ("-5 minutes", make_timed_session(start, -5, "s")),
        ("the smallest number of minutes", make_timed_session(start, i64::MIN, "s")),
        ("one over 7 days", make_timed_session(start, MAX_SESSION_MINUTES + 1, "s")),
        ("a huge number of minutes", make_timed_session(start, i64::MAX, "s")),
        ("a moment near the largest number", make_timed_session(i64::MAX - 1000, 60, "s")),
        ("the largest moment", make_timed_session(i64::MAX, 1, "s")),
        ("the smallest moment", make_timed_session(i64::MIN, 1, "s")),
        ("an end past the year 9999", make_timed_session(LAST_SECOND_OF_9999_MS, 1, "s")),
        ("a moment past the year 9999", make_timed_session(LAST_SECOND_OF_9999_MS + 60_000, 1, "s")),
        ("a rule id that is empty", make_timed_session(start, 5, "")),
        ("a rule id with a space", make_timed_session(start, 5, "my session")),
        ("a rule id with a slash", make_timed_session(start, 5, "a/b")),
        ("a rule id of 65 characters", make_timed_session(start, 5, &"a".repeat(65))),
    ];
    for (label, result) in &refused {
        match result {
            Ok(session) => panic!("{label} was made: {session:?}"),
            Err(sentence) => assert!(sentence.ends_with('.') && sentence.len() > 20, "{label}: a sentence is given, not '{sentence}'"),
        }
    }
    // The edges that do fit are made: the last second of year 9999, and a 64-character id.
    assert_eq!(made(LAST_SECOND_OF_9999_MS - MINUTE_MS, 1).ends_at, "9999-12-31T23:59:59Z");
    assert!(make_timed_session(start, 5, &"a".repeat(64)).is_ok());
    let sentence = make_timed_session(start, 0, "s").expect_err("refused");
    assert!(sentence.contains('0') && sentence.contains("minute"), "the sentence says what was asked: {sentence}");
    let sentence = make_timed_session(start, 10_081, "s").expect_err("refused");
    assert!(sentence.contains("10080") && sentence.contains("10081"), "the sentence says the limit and what was asked: {sentence}");
}
