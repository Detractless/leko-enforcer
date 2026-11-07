//! Each rule kind means what the draft says (AC-004): the block is drafted, added to a settings zip,
//! read back through the real readers, and the decision is asked at moments inside and outside.

mod block_builder;
mod zip_writer;

use block_builder::{moment, none_blocked, zone};
use leko_core::blocks::add_block::add_block;
use leko_core::blocks::block_request::{BlockRequest, DoorChoice, RuleChoice};
use leko_core::blocks::draft_block::draft_block;
use leko_core::decision::block_cover::Target;
use leko_core::decision::decide_target::decide;
use leko_core::io::zip_check::{check_zip, ZipVerdict};
use leko_core::models::specs::{Lock, ReadSet};
use leko_core::sessions::utc_moment::format_utc_moment;
use std::collections::HashSet;
use zip_writer::zip_of;

const APP: &str = "com.example.app";
const SITE: &str = "example.com";

fn request(rule: RuleChoice, days: &[&str]) -> BlockRequest {
    BlockRequest {
        name: "Focus".to_string(),
        apps: vec![APP.to_string()],
        sites: vec![SITE.to_string()],
        rule,
        days: days.iter().map(|day| day.to_string()).collect(),
        door: DoorChoice::Default,
        message: None,
    }
}

fn hours(stretches: &[(&str, &str)]) -> RuleChoice {
    RuleChoice::OpenHours(stretches.iter().map(|(from, to)| (from.to_string(), to.to_string())).collect())
}

/// The settings that hold the block `ask` makes at `now_ms`, read back from the zip it was added to.
fn read_back(ask: &BlockRequest, now_ms: i64) -> ReadSet {
    let draft = draft_block(ask, now_ms).unwrap_or_else(|sentence| panic!("{sentence}"));
    let base = zip_of(&[("GUIDE.md", "guide"), ("general.json", r#"{"schema": 2}"#)]);
    let added = add_block(&base, &draft).unwrap_or_else(|problem| panic!("{}", problem.sentence()));
    match check_zip(&added.bytes, &HashSet::new()) {
        ZipVerdict::Read(read) => read.set,
        ZipVerdict::Refused(refusal) => panic!("the saved zip is refused: {}", refusal.sentence),
    }
}

/// Whether the app and the site are closed at the RFC 3339 `at`, on UTC or the given zone's wall clock.
fn closed(set: &ReadSet, zone_name: &str, at: &str) -> bool {
    let (by_app, by_site) = (
        decide(set, Target::App(APP), moment(at), &zone(zone_name), &none_blocked()).closed,
        decide(set, Target::Host(SITE), moment(at), &zone(zone_name), &none_blocked()).closed,
    );
    assert_eq!(by_app, by_site, "the app and the site of one block are judged alike at {at}");
    by_app
}

fn verdicts(set: &ReadSet, zone_name: &str, moments: &[&str]) -> Vec<bool> {
    moments.iter().map(|at| closed(set, zone_name, at)).collect()
}

#[test]
fn each_rule_kind_reads_back_and_decides_as_the_words_say() {
    let now = moment("2026-11-05T15:00:00Z");
    // 2026-11-02 is a Monday; noon UTC of each day of that week.
    let week = ["2026-11-02T12:00:00Z", "2026-11-03T12:00:00Z", "2026-11-04T12:00:00Z", "2026-11-05T12:00:00Z", "2026-11-06T12:00:00Z", "2026-11-07T12:00:00Z", "2026-11-08T12:00:00Z"];
    let all_days = ["mon", "tue", "wed", "thu", "fri", "sat", "sun"];

    // --always: every day, all day, and the same for each weekday named on its own or in a list.
    let always = read_back(&request(RuleChoice::Always, &[]), now);
    assert_eq!(verdicts(&always, "UTC", &week), [true; 7]);
    assert!(closed(&always, "UTC", "2026-11-05T00:00:00Z") && closed(&always, "UTC", "2026-11-05T23:59:59Z"));
    for (index, day) in all_days.iter().enumerate() {
        let mut want = [false; 7];
        want[index] = true;
        assert_eq!(verdicts(&read_back(&request(RuleChoice::Always, &[day]), now), "UTC", &week), want, "--days {day}");
    }
    let mixed = read_back(&request(RuleChoice::Always, &["sun", "tue", "mon"]), now);
    assert_eq!(verdicts(&mixed, "UTC", &week), [true, true, false, false, false, false, true], "days given out of order");
    assert_eq!(verdicts(&read_back(&request(RuleChoice::Always, &all_days), now), "UTC", &week), [true; 7], "all seven days");
    // The day is the wall clock's day: Sunday 23:30 in New York is already Monday in UTC.
    let mondays = read_back(&request(RuleChoice::Always, &["mon"]), now);
    assert!(!closed(&mondays, "America/New_York", "2026-11-01T23:30:00-05:00"));
    assert!(closed(&mondays, "UTC", "2026-11-01T23:30:00-05:00"));

    // --open-hours 09:00-17:00: open inside, closed at every other time.
    let office = read_back(&request(hours(&[("09:00", "17:00")]), &[]), now);
    let at = |time: &str| format!("2026-11-05T{time}:00Z");
    for (time, want) in [("00:00", true), ("03:00", true), ("08:59", true), ("09:00", false), ("12:00", false), ("16:59", false), ("17:00", true), ("20:00", true), ("23:59", true)] {
        assert_eq!(closed(&office, "UTC", &at(time)), want, "09:00-17:00 at {time}");
    }
    assert!(closed(&office, "UTC", "2026-11-07T03:00:00Z") && !closed(&office, "UTC", "2026-11-07T12:00:00Z"), "every day when no weekday is named");

    // Two stretches, one ending at 24:00 (the end of the day is not lost).
    let split = read_back(&request(hours(&[("09:00", "12:00"), ("19:00", "24:00")]), &[]), now);
    for (time, want) in [("08:00", true), ("10:00", false), ("12:00", true), ("15:00", true), ("19:00", false), ("23:59", false), ("00:00", true)] {
        assert_eq!(closed(&split, "UTC", &at(time)), want, "09:00-12:00,19:00-24:00 at {time}");
    }

    // A stretch crossing midnight is open on both sides of it.
    let night = read_back(&request(hours(&[("22:00", "02:00")]), &[]), now);
    for (time, want) in [("21:59", true), ("22:00", false), ("23:30", false), ("00:30", false), ("01:59", false), ("02:00", true), ("12:00", true)] {
        assert_eq!(closed(&night, "UTC", &at(time)), want, "22:00-02:00 at {time}");
    }

    // Hours with weekdays: the rule only applies on those days, so other days are not closed by it.
    let monday_office = read_back(&request(hours(&[("09:00", "17:00")]), &["mon"]), now);
    assert!(closed(&monday_office, "UTC", "2026-11-02T03:00:00Z"), "Monday outside the stretch");
    assert!(!closed(&monday_office, "UTC", "2026-11-02T12:00:00Z"), "Monday inside the stretch");
    assert!(!closed(&monday_office, "UTC", "2026-11-03T03:00:00Z"), "Tuesday is not covered by the rule");

    // --for 90m: closed from the moment of the request until the end, which is the moment plus the length.
    let timed_ask = request(RuleChoice::ForMinutes(90), &[]);
    let draft = draft_block(&timed_ask, now).unwrap();
    let ends_ms = now + 90 * 60_000;
    assert_eq!(draft.ends_at, format_utc_moment(ends_ms));
    let timed = read_back(&timed_ask, now);
    for (time, want) in [("2026-11-05T15:00:00Z", true), ("2026-11-05T16:29:59Z", true), ("2026-11-05T16:30:00Z", false), ("2026-11-05T17:00:00Z", false)] {
        assert_eq!(closed(&timed, "UTC", time), want, "--for 90m at {time}");
    }
    let spec = timed.blocks.get("focus").expect("the block is in the set");
    assert_eq!(spec.rules.len(), 1);
    assert_eq!(spec.rules[0].ends_at, Some(ends_ms), "the session ends at the moment plus the length");
    assert!(matches!(&spec.lock, Lock::SealedAt { at, .. } if *at == ends_ms), "the seal ends with the session: {:?}", spec.lock);

    // Every length `leko session` accepts, and no other.
    for minutes in [1, 60, 1440, 10_080] {
        assert!(draft_block(&request(RuleChoice::ForMinutes(minutes), &[]), now).is_ok(), "{minutes} minutes");
    }
    for minutes in [0, -5, 10_081] {
        assert!(draft_block(&request(RuleChoice::ForMinutes(minutes), &[]), now).is_err(), "{minutes} minutes");
    }
    let mut with_days = request(RuleChoice::ForMinutes(30), &["mon"]);
    assert!(draft_block(&with_days, now).is_err(), "a timed block has no weekdays");
    with_days.days.clear();
    assert!(draft_block(&with_days, now).is_ok());
}
