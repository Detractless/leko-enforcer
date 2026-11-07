//! A moment seal is a lock of level `sealedAt` with `at` written like a session's `endsAt`: not
//! before that exact UTC moment. It is read and judged like a date seal but by the instant it ends
//! (AC-007), and a loosening requested under it lands at that instant, not at the next midnight (AC-008).

mod extension_cases;
mod vector_files;
mod zip_writer;

use extension_cases::{diff_problems, landing, load, instant, ZONES};
use leko_core::config_time::zone_time::ZoneTime;
use leko_core::io::zip_check::{check_zip, Refusal, ZipVerdict};
use leko_core::models::specs::Lock;
use serde_json::{json, Value};
use std::collections::HashSet;
use zip_writer::zip_of;

const DIFF_CASES: usize = 27;
const DUE_CASES: usize = 15;
const SAME_IN_EVERY_ZONE: usize = 9;

fn block_file(id: &str, lock: &Value) -> String {
    let rules = json!([{"id": "m", "kind": "minutes", "amount": 30, "per": "day", "scope": "block"}]);
    json!({"schema": 2, "id": id, "name": "B", "covers": {"apps": ["com.b"]}, "rules": rules, "lock": lock}).to_string()
}

fn verdict_of(lock: &Value) -> ZipVerdict {
    let good = block_file("a", &json!({"level": "open"}));
    let under_test = block_file("b", lock);
    check_zip(&zip_of(&[("GUIDE.md", "guide"), ("blocks/a.json", &good), ("blocks/b.json", &under_test)]), &HashSet::new())
}

fn lock_read(lock: &Value) -> Lock {
    match verdict_of(lock) {
        ZipVerdict::Read(read) => read.set.blocks.get("b").unwrap_or_else(|| panic!("block b is missing")).lock.clone(),
        ZipVerdict::Refused(refusal) => panic!("expected {lock} to be read, but it was refused: {}", refusal.sentence),
    }
}

fn refusal_of(lock: &Value) -> Refusal {
    match verdict_of(lock) {
        ZipVerdict::Refused(refusal) => refusal,
        ZipVerdict::Read(_) => panic!("expected a refusal for {lock}, but the zip was read"),
    }
}

fn seal(at: &str) -> Value {
    json!({"level": "sealedAt", "at": at})
}

#[test]
fn moment_seal_changes_are_judged_like_date_seals() {
    // Read: the moment becomes an instant; one that has already passed is read all the same.
    assert_eq!(lock_read(&seal("2026-12-01T00:00:00Z")), Lock::SealedAt { at: instant("2026-12-01T00:00:00Z"), key: None });
    assert_eq!(lock_read(&seal("2026-02-28T23:59:59Z")), Lock::SealedAt { at: instant("2026-02-28T23:59:59Z"), key: None });
    assert_eq!(lock_read(&seal("1970-01-01T00:00:00Z")), Lock::SealedAt { at: 0, key: None });
    assert_eq!(lock_read(&seal("2028-02-29T00:00:00Z")), Lock::SealedAt { at: instant("2028-02-29T00:00:00Z"), key: None });

    // Refused whole, naming blocks/b.json and the field.
    let bad: Vec<(&str, Value, &str)> = vec![
        ("no at", json!({"level": "sealedAt"}), "at"),
        ("an offset", seal("2026-12-01T00:00:00+01:00"), "at"),
        ("no Z", seal("2026-12-01T00:00:00"), "at"),
        ("a part of a second", seal("2026-12-01T00:00:00.5Z"), "at"),
        ("a date only", seal("2026-12-01"), "at"),
        ("February 30", seal("2026-02-30T00:00:00Z"), "at"),
        ("hour 24", seal("2026-12-01T24:00:00Z"), "at"),
        ("second 60", seal("2026-12-01T00:00:60Z"), "at"),
        ("an empty text", seal(""), "at"),
        ("a number", json!({"level": "sealedAt", "at": 1_793_000_000}), "at"),
        ("null", json!({"level": "sealedAt", "at": null}), "at"),
        ("until beside at", json!({"level": "sealedAt", "at": "2026-12-01T00:00:00Z", "until": "2026-12-01"}), "until"),
        ("hours beside at", json!({"level": "sealedAt", "at": "2026-12-01T00:00:00Z", "hours": 5}), "hours"),
        ("at on a date seal", json!({"level": "sealedUntil", "until": "2026-12-01", "at": "2026-12-01T00:00:00Z"}), "at"),
        ("at on an open lock", json!({"level": "open", "at": "2026-12-01T00:00:00Z"}), "at"),
    ];
    for (label, lock, field) in &bad {
        let refusal = refusal_of(lock);
        assert_eq!(refusal.entry.as_deref(), Some("blocks/b.json"), "{label}: the refusal names the file");
        assert!(refusal.sentence.starts_with("blocks/b.json: "), "{label}: {}", refusal.sentence);
        assert!(refusal.sentence.contains(&format!("\"{field}\"")) || refusal.sentence.contains(&format!("lock.{field}")), "{label}: the sentence names {field}: {}", refusal.sentence);
        let marked: Vec<&str> = refusal.files.iter().filter(|file| file.problem.is_some()).map(|file| file.path.as_str()).collect();
        assert_eq!(marked, vec!["blocks/b.json"], "{label}: only the bad file carries the problem");
    }

    // Judged: the listed change cases, with the lock level read from the extended schema.
    let file = load("seal-cases.json");
    assert_eq!(diff_problems(&file, DIFF_CASES), Vec::<String>::new());
}

#[test]
fn due_time_under_a_moment_seal_is_the_exact_moment() {
    let file = load("seal-cases.json");
    let cases = file.doc["due"].as_array().expect("due");
    assert_eq!(cases.len(), DUE_CASES);
    let wrong: Vec<String> = cases
        .iter()
        .filter_map(|item| {
            let want = item["expect"].as_str().map(instant);
            let got = landing(item, &file.zone);
            (got != want).then(|| format!("{}: expected {want:?}, got {got:?}", item["name"]))
        })
        .collect();
    assert_eq!(wrong, Vec::<String>::new());

    // A moment seal answers the same instant in any zone; a date seal does not.
    let mut checked = 0;
    for item in cases.iter().filter(|item| item["sameInEveryZone"] == true) {
        let want = item["expect"].as_str().map(instant);
        for name in ZONES {
            let zone = ZoneTime::new(name).expect("zone");
            assert_eq!(landing(item, &zone), want, "{} in {name}", item["name"]);
        }
        checked += 1;
    }
    assert_eq!(checked, SAME_IN_EVERY_ZONE);
    let date_seal = json!({"name": "a date seal", "at": "2026-11-02T10:00:00-05:00", "locks": [{"level": "sealedUntil", "until": "2026-11-03"}]});
    let tokyo = ZoneTime::new("Asia/Tokyo").expect("zone");
    assert_ne!(landing(&date_seal, &tokyo), landing(&date_seal, &file.zone), "a date seal ends at the zone's midnight");
}
