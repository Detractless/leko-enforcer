//! What each time rule says at an instant, on the wall clock of the given zone: weekday rules
//! (AC-002), outside-hours stretches with the phone's own cases (AC-003), a session until its end
//! moment (AC-004), and the 23-hour and 25-hour days of a clock change (AC-005).

mod block_builder;
mod zip_writer;

use block_builder::{always, block, every_open, moment, none_blocked, outside_hours, read_set, session, zone};
use leko_core::decision::block_cover::Target;
use leko_core::decision::decide_target::decide;
use leko_core::models::specs::ReadSet;
use serde_json::{json, Value};

const APP: &str = "com.a";
const NEW_YORK: &str = "America/New_York";

fn set_with(rule: Value) -> ReadSet {
    read_set(&[block("b", json!({"apps": [APP]}), json!([rule]))])
}

/// Whether the app is closed at the RFC 3339 `at`, judged on the wall clock of `zone_name`.
fn closed(set: &ReadSet, zone_name: &str, at: &str) -> bool {
    decide(set, Target::App(APP), moment(at), &zone(zone_name), &none_blocked()).closed
}

fn verdicts(set: &ReadSet, zone_name: &str, moments: &[&str]) -> Vec<bool> {
    moments.iter().map(|at| closed(set, zone_name, at)).collect()
}

#[test]
fn always_and_every_open_close_on_the_named_weekdays_in_the_given_zone() {
    for kind in [always as fn(&str, &[&str]) -> Value, every_open] {
        let mondays = set_with(kind("r", &["mon"]));
        // Monday and Tuesday noon in New York (2026-11-02 is a Monday).
        assert!(closed(&mondays, NEW_YORK, "2026-11-02T12:00:00-05:00"), "Monday");
        assert!(!closed(&mondays, NEW_YORK, "2026-11-03T12:00:00-05:00"), "Tuesday");
        // 23:30 on Sunday in New York is already Monday in UTC: the zone's day counts, not UTC's.
        let sunday_night = "2026-11-01T23:30:00-05:00";
        assert!(!closed(&mondays, NEW_YORK, sunday_night), "Sunday night in New York");
        assert!(closed(&mondays, "UTC", sunday_night), "the same instant is Monday in UTC");
        // 20:00 UTC on Monday is already Tuesday 05:00 in Tokyo.
        assert!(closed(&mondays, "UTC", "2026-11-02T20:00:00Z"));
        assert!(!closed(&mondays, "Asia/Tokyo", "2026-11-02T20:00:00Z"), "Tuesday in Tokyo");

        let weekend = set_with(kind("r", &["sat", "sun"]));
        let week = ["2026-11-02T12:00:00-05:00", "2026-11-03T12:00:00-05:00", "2026-11-04T12:00:00-05:00", "2026-11-05T12:00:00-05:00", "2026-11-06T12:00:00-05:00", "2026-11-07T12:00:00-05:00", "2026-11-08T12:00:00-05:00"];
        assert_eq!(verdicts(&weekend, NEW_YORK, &week), [false, false, false, false, false, true, true]);

        let every_day = set_with(kind("r", &[]));
        assert_eq!(verdicts(&every_day, NEW_YORK, &week), [true; 7], "no weekday named means every day");
        let all_seven = set_with(kind("r", &["mon", "tue", "wed", "thu", "fri", "sat", "sun"]));
        assert_eq!(verdicts(&all_seven, NEW_YORK, &week), [true; 7], "all seven named is every day");
    }
}

#[test]
fn outside_hours_follows_the_phone_across_midnight_and_weekdays() {
    // The phone's case: allowed 22:00 to 06:00 on Mondays only.
    let mondays = set_with(outside_hours("r", &["mon"], &[("22:00", "06:00")]));
    let monday_23 = "2026-11-02T23:00:00-05:00";
    let monday_noon = "2026-11-02T12:00:00-05:00";
    let tuesday_03 = "2026-11-03T03:00:00-05:00";
    let tuesday_noon = "2026-11-03T12:00:00-05:00";
    let monday_01 = "2026-11-02T01:00:00-05:00";
    assert!(!closed(&mondays, NEW_YORK, monday_23), "Monday 23:00 is inside the stretch");
    assert!(closed(&mondays, NEW_YORK, monday_noon), "Monday 12:00 is outside the stretch");
    assert!(!closed(&mondays, NEW_YORK, tuesday_03), "Tuesday 03:00 is the part after midnight of Monday's stretch");
    assert!(!closed(&mondays, NEW_YORK, tuesday_noon), "the rule does not apply on Tuesday");
    assert!(closed(&mondays, NEW_YORK, monday_01), "Monday 01:00 is not Sunday's stretch, since Sunday is not named");

    // An empty allowed list is the same as always.
    let empty = set_with(outside_hours("r", &[], &[]));
    assert_eq!(verdicts(&empty, NEW_YORK, &[monday_noon, monday_23, tuesday_03]), [true, true, true]);

    // A stretch that runs to 24:00 closes at midnight, and its end is not part of it.
    let to_midnight = set_with(outside_hours("r", &[], &[("06:00", "24:00")]));
    assert_eq!(verdicts(&to_midnight, NEW_YORK, &["2026-11-02T23:59:00-05:00", "2026-11-03T00:00:00-05:00", "2026-11-03T05:59:00-05:00", "2026-11-03T06:00:00-05:00"]), [false, true, true, false]);

    // Every day, a stretch across midnight: the start is inside, the end is outside.
    let nights = set_with(outside_hours("r", &[], &[("22:00", "06:00")]));
    let edges = ["2026-11-02T21:59:00-05:00", "2026-11-02T22:00:00-05:00", "2026-11-03T05:59:00-05:00", "2026-11-03T06:00:00-05:00", "2026-11-03T12:00:00-05:00"];
    assert_eq!(verdicts(&nights, NEW_YORK, &edges), [true, false, false, true, true]);

    // Two stretches: a start is inside, an end is outside.
    let two = set_with(outside_hours("r", &[], &[("06:00", "09:00"), ("12:00", "13:00")]));
    let day = ["05:59", "06:00", "08:59", "09:00", "12:30", "13:00"].map(|time| format!("2026-11-02T{time}:00-05:00"));
    let day: Vec<&str> = day.iter().map(String::as_str).collect();
    assert_eq!(verdicts(&two, NEW_YORK, &day), [true, false, false, true, false, true]);

    // When today and yesterday are both named, the part after midnight of yesterday's stretch is
    // allowed; when only today is named, yesterday's stretch does not exist and 03:00 is outside.
    let mon_tue = set_with(outside_hours("r", &["mon", "tue"], &[("22:00", "06:00")]));
    let tue_wed = set_with(outside_hours("r", &["tue", "wed"], &[("22:00", "06:00")]));
    assert!(!closed(&mon_tue, NEW_YORK, tuesday_03), "Monday and Tuesday named: Tuesday 03:00 is Monday's stretch");
    assert!(closed(&tue_wed, NEW_YORK, tuesday_03), "Tuesday and Wednesday named: Monday is not, so Tuesday 03:00 is outside");
    assert!(!closed(&tue_wed, NEW_YORK, "2026-11-04T03:00:00-05:00"), "Wednesday 03:00 is Tuesday's stretch");

    // A Friday stretch past midnight is Friday's, so Saturday 03:00 is inside it and Saturday 12:00 is not judged by it.
    let fridays = set_with(outside_hours("r", &["fri"], &[("22:00", "06:00")]));
    assert_eq!(verdicts(&fridays, NEW_YORK, &["2026-11-07T03:00:00-05:00", "2026-11-07T12:00:00-05:00", "2026-11-06T12:00:00-05:00"]), [false, false, true]);
}

#[test]
fn a_session_closes_until_its_end_moment_and_not_after() {
    let end = moment("2026-11-05T18:30:00Z");
    let set = set_with(session("s", "2026-11-05T18:30:00Z"));
    let ask = |zone_name: &str, at: i64| decide(&set, Target::App(APP), at, &zone(zone_name), &none_blocked()).closed;
    for zone_name in ["UTC", NEW_YORK, "Asia/Tokyo"] {
        assert!(ask(zone_name, end - 1), "{zone_name}: one millisecond before the end is closed");
        assert!(!ask(zone_name, end), "{zone_name}: the end moment itself is open");
        assert!(!ask(zone_name, end + 1), "{zone_name}: after the end is open");
        assert!(!ask(zone_name, end + 400 * 86_400_000), "{zone_name}: long after is open");
    }
    // Every weekday of the week before the end is closed: a session has no weekdays.
    for day in 1..=7 {
        assert!(ask("UTC", end - day * 86_400_000), "{day} days before the end");
    }
    let started = moment("2026-11-05T18:00:00Z");
    assert!(ask("UTC", started) && ask("UTC", end - 1_000));
}

#[test]
fn clock_changes_keep_local_minutes() {
    // The phone's own cases: allowed 06:00 to 23:00 on the 23-hour and the 25-hour day.
    let daytime = set_with(outside_hours("r", &[], &[("06:00", "23:00")]));
    assert!(!closed(&daytime, NEW_YORK, "2026-03-08T07:00:00-04:00"));
    assert!(closed(&daytime, NEW_YORK, "2026-03-08T23:30:00-04:00"));
    assert!(!closed(&daytime, NEW_YORK, "2026-11-01T22:59:00-05:00"));
    assert!(closed(&daytime, NEW_YORK, "2026-11-01T23:00:00-05:00"));

    // The repeated hour of autumn shows 01:30 twice (05:30Z and 06:30Z); both are 01:30 on the wall.
    let quarter_past = set_with(outside_hours("r", &[], &[("01:15", "01:45")]));
    assert_eq!(verdicts(&quarter_past, NEW_YORK, &["2026-11-01T05:30:00Z", "2026-11-01T06:30:00Z"]), [false, false], "01:30 is inside 01:15 to 01:45 on both passes");
    let early = set_with(outside_hours("r", &[], &[("01:00", "01:20")]));
    assert_eq!(verdicts(&early, NEW_YORK, &["2026-11-01T05:30:00Z", "2026-11-01T06:30:00Z"]), [true, true], "01:30 is outside 01:00 to 01:20 on both passes");

    // The skipped hour of spring: at 07:00Z the wall clock jumps from 01:59 to 03:00. A stretch that
    // starts at 02:30, a minute that does not exist that day, is judged by the clock after the jump.
    let late_start = set_with(outside_hours("r", &[], &[("02:30", "04:00")]));
    let moments = ["2026-03-08T06:59:00Z", "2026-03-08T07:00:00Z", "2026-03-08T07:30:00Z", "2026-03-08T07:59:00Z", "2026-03-08T08:00:00Z"];
    assert_eq!(verdicts(&late_start, NEW_YORK, &moments), [true, false, false, false, true], "01:59 closed, 03:00 to 03:59 inside, 04:00 closed");
}
