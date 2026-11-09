//! The next flip is the moment a caller waits for when it asks "until when": the first instant after
//! the given one at which one app or one site goes from closed to open or back (AC-006). Every answer
//! is also checked against the decision itself: the target's answer is the same at every moment
//! sampled before it, and differs at it.

mod block_builder;
mod zip_writer;

use block_builder::{always, block, block_with, minutes, moment, none_blocked, one_rule, opens, outside_hours, read_set, session, zone, APP, DAY_MS, NEW_YORK};
use leko_core::decision::block_cover::Target;
use leko_core::decision::decide_target::decide;
use leko_core::decision::next_change::next_change;
use leko_core::models::specs::ReadSet;
use leko_core::status::next_flip::next_flip;
use serde_json::json;
use std::collections::HashSet;

fn closed_at(set: &ReadSet, target: Target, never: &HashSet<String>, zone_name: &str, at_ms: i64) -> bool {
    decide(set, target, at_ms, &zone(zone_name), never).closed
}

/// Asks for the next flip after `from` and checks it is `expected`, then asks the decision itself:
/// every 20 seconds before the answer (every 5 minutes for no answer) the target is as it was at
/// `from`, one millisecond before the answer it still is, and at the answer it is not.
fn check(what: &str, set: &ReadSet, target: Target, never: &HashSet<String>, zone_name: &str, from: i64, expected: Option<i64>) {
    let answer = next_flip(set, target, never, from, &zone(zone_name));
    assert_eq!(answer, expected, "{what}");
    assert!(answer.is_none_or(|at| at > from), "{what}: the answer is not after the question");
    let start = closed_at(set, target, never, zone_name, from);
    let (end, step) = match expected {
        Some(at) => (at, 20_000),
        None => (from + 8 * DAY_MS, 300_000),
    };
    let mut sample = from + step;
    while sample < end {
        assert_eq!(closed_at(set, target, never, zone_name, sample), start, "{what}: the answer flipped before the answer, at {sample}");
        sample += step;
    }
    if let Some(at) = expected {
        assert_eq!(closed_at(set, target, never, zone_name, at - 1), start, "{what}: the target flipped within a millisecond before the answer");
        assert_ne!(closed_at(set, target, never, zone_name, at), start, "{what}: nothing flips at the answer");
    }
}

fn expect_app(what: &str, set: &ReadSet, zone_name: &str, from: i64, expected: Option<i64>) {
    check(what, set, Target::App(APP), &none_blocked(), zone_name, from, expected);
}

#[test]
fn the_next_flip_is_the_first_moment_the_answer_for_that_target_differs() {
    // A stretch edge is that minute, however far into the minute the question comes; at the edge
    // itself the answer is already the new one, so the answer is the flip after it.
    let office = one_rule(outside_hours("r", &[], &[("09:00", "17:00")]));
    let five = Some(moment("2026-11-05T17:00:00-05:00"));
    let nine = Some(moment("2026-11-06T09:00:00-05:00"));
    expect_app("hours, from 10:00 (open, then closed)", &office, NEW_YORK, moment("2026-11-05T10:00:00-05:00"), five);
    expect_app("hours, from 10:00:30", &office, NEW_YORK, moment("2026-11-05T10:00:30-05:00"), five);
    expect_app("hours, from the last millisecond", &office, NEW_YORK, moment("2026-11-05T16:59:59.999-05:00"), five);
    expect_app("hours, from the edge", &office, NEW_YORK, moment("2026-11-05T17:00:00-05:00"), nine);
    expect_app("hours, from 18:00 (closed, then open)", &office, NEW_YORK, moment("2026-11-05T18:00:00-05:00"), nine);

    // Another block's edge is not this target's: the block for com.other opens at 11:00, the one for
    // the app closes at 17:00. The next change of anything is 11:00; the next flip of the app is 17:00.
    let two_blocks = read_set(&[
        block("other", json!({"apps": ["com.other"]}), json!([outside_hours("r", &[], &[("11:00", "12:00")])])),
        block("mine", json!({"apps": [APP]}), json!([outside_hours("r", &[], &[("09:00", "17:00")])])),
    ]);
    let ten = moment("2026-11-05T10:00:00-05:00");
    assert_eq!(next_change(&two_blocks, ten, &zone(NEW_YORK)), Some(moment("2026-11-05T11:00:00-05:00")), "the case needs an unrelated edge to ignore");
    expect_app("an unrelated block's edge first", &two_blocks, NEW_YORK, ten, five);
    check("the other app, same settings", &two_blocks, Target::App("com.other"), &none_blocked(), NEW_YORK, ten, Some(moment("2026-11-05T11:00:00-05:00")));

    // A rule that opens while another still closes is not a flip. Monday is closed all day by the
    // first rule; the second closes outside 09:00 to 17:00. The next change is 17:00 on Monday, but the
    // target stays closed through Monday night and Tuesday morning until both are open, at 09:00 Tuesday.
    let layered = read_set(&[block("b", json!({"apps": [APP]}), json!([always("monday", &["mon"]), outside_hours("hours", &[], &[("09:00", "17:00")])]))]);
    let monday_ten = moment("2026-11-02T10:00:00-05:00");
    assert_eq!(next_change(&layered, monday_ten, &zone(NEW_YORK)), Some(moment("2026-11-02T17:00:00-05:00")), "the case needs a change that is not a flip");
    expect_app("closed under two rules", &layered, NEW_YORK, monday_ten, Some(moment("2026-11-03T09:00:00-05:00")));
    expect_app("a rule opens under a rule that still closes", &layered, NEW_YORK, moment("2026-11-02T23:00:00-05:00"), Some(moment("2026-11-03T09:00:00-05:00")));

    // A session ending under a rule that still closes is not a flip either.
    let covered_end = read_set(&[block("b", json!({"apps": [APP]}), json!([outside_hours("hours", &[], &[("13:00", "23:00")]), session("s", "2026-11-05T12:00:00Z")]))]);
    assert_eq!(next_change(&covered_end, moment("2026-11-05T10:00:00Z"), &zone("UTC")), Some(moment("2026-11-05T12:00:00Z")), "the case needs a session end that is not a flip");
    expect_app("a session ends under a closing rule", &covered_end, "UTC", moment("2026-11-05T10:00:00Z"), Some(moment("2026-11-05T13:00:00Z")));
    // A session end between two minutes that is a flip, though the same rule closes again a few seconds later.
    let gap = read_set(&[block("b", json!({"apps": [APP]}), json!([outside_hours("hours", &[], &[("08:00", "12:01")]), session("s", "2026-11-05T12:00:30Z")]))]);
    expect_app("a session end that is a flip between two minutes", &gap, "UTC", moment("2026-11-05T11:00:00Z"), Some(moment("2026-11-05T12:00:30Z")));

    // A session end is that second, and is found only within 8 days of the question.
    let end = moment("2026-11-13T10:00:00Z");
    let ends = one_rule(session("s", "2026-11-13T10:00:00Z"));
    expect_app("session end, exactly 8 days ahead", &ends, "UTC", end - 8 * DAY_MS, Some(end));
    expect_app("session end, just under 8 days ahead", &ends, "UTC", end - 8 * DAY_MS + 1, Some(end));
    expect_app("session end, 8 days and a millisecond ahead", &ends, "UTC", end - 8 * DAY_MS - 1, None);
    expect_app("session end, 9 days ahead", &ends, "UTC", end - 9 * DAY_MS, None);
    expect_app("session end, asked at the end", &ends, "UTC", end, None);
    expect_app("session end, asked after the end", &ends, "UTC", end + DAY_MS, None);
    let off_beat = moment("2026-11-13T10:00:01Z");
    let off_beat_rule = one_rule(session("s", "2026-11-13T10:00:01Z"));
    expect_app("session end 8 days ahead, between minutes", &off_beat_rule, "UTC", off_beat - 8 * DAY_MS, Some(off_beat));
    expect_app("session end 8 days and a millisecond ahead, between minutes", &off_beat_rule, "UTC", off_beat - 8 * DAY_MS - 1, None);
    let in_two_days = one_rule(session("s", "2026-11-07T18:30:01Z"));
    expect_app("a session ending in two days", &in_two_days, NEW_YORK, moment("2026-11-05T18:30:01Z"), Some(moment("2026-11-07T18:30:01Z")));
    expect_app("asked in the second the session ended", &in_two_days, NEW_YORK, moment("2026-11-07T18:30:01.500Z"), None);

    // The earliest flip of the covering blocks wins; a switched-off block and a block for another app are not looked at.
    let mixed = read_set(&[
        block("hours", json!({"apps": [APP]}), json!([outside_hours("r", &[], &[("09:00", "17:00")])])),
        block("short", json!({"apps": [APP]}), json!([session("s", "2026-11-05T12:00:00Z")])),
        block_with("off", json!({"apps": [APP]}), json!([outside_hours("r", &[], &[("10:30", "11:00")])]), json!({"on": false})),
        block("elsewhere", json!({"apps": ["com.other"]}), json!([session("s", "2026-11-05T11:00:00Z")])),
    ]);
    expect_app("two blocks, an off block and another app's block", &mixed, "UTC", moment("2026-11-05T10:00:00Z"), Some(moment("2026-11-05T12:00:00Z")));
}

#[test]
fn the_flip_follows_the_zones_calendar_and_clock_changes() {
    // A rule on one weekday flips at the zone's midnights.
    let mondays = one_rule(always("r", &["mon"]));
    expect_app("Monday rule, from Tuesday", &mondays, NEW_YORK, moment("2026-11-03T12:00:00-05:00"), Some(moment("2026-11-09T00:00:00-05:00")));
    expect_app("Monday rule, from Monday", &mondays, NEW_YORK, moment("2026-11-09T12:00:00-05:00"), Some(moment("2026-11-10T00:00:00-05:00")));

    // Seven days and an hour away, across the repeated hour of autumn.
    let last_minute = one_rule(outside_hours("r", &["mon"], &[("00:00", "23:59")]));
    let from = moment("2026-10-27T00:00:30-04:00");
    let answer = moment("2026-11-02T23:59:00-05:00");
    assert!(answer - from > 7 * DAY_MS, "the case must reach past a week");
    expect_app("a week and an hour away", &last_minute, NEW_YORK, from, Some(answer));

    // The spring jump is the flip; the repeated autumn hour is judged by the wall clock both times.
    let late_start = one_rule(outside_hours("r", &[], &[("02:30", "04:00")]));
    expect_app("spring jump", &late_start, NEW_YORK, moment("2026-03-08T06:59:00Z"), Some(moment("2026-03-08T07:00:00Z")));
    let quarter = one_rule(outside_hours("r", &[], &[("01:15", "01:45")]));
    expect_app("autumn, first pass", &quarter, NEW_YORK, moment("2026-11-01T05:40:00Z"), Some(moment("2026-11-01T05:45:00Z")));
    expect_app("autumn, between the passes", &quarter, NEW_YORK, moment("2026-11-01T05:50:00Z"), Some(moment("2026-11-01T06:15:00Z")));

    // The same instant is a different day in another zone.
    let tokyo_monday = one_rule(always("r", &["mon"]));
    expect_app("Monday rule in Tokyo", &tokyo_monday, "Asia/Tokyo", moment("2026-11-02T20:00:00Z"), Some(moment("2026-11-08T15:00:00Z")));
}

#[test]
fn apps_and_sites_have_their_own_flips_and_nothing_flips_for_what_is_never_applied() {
    let hours = outside_hours("r", &[], &[("09:00", "17:00")]);
    let ten = moment("2026-11-05T10:00:00Z");
    let seventeen = Some(moment("2026-11-05T17:00:00Z"));
    let nobody = none_blocked();

    // A site is judged by its own blocks: a subdomain of a listed name flips with it, another host and
    // every app do not.
    let sites = read_set(&[
        block("site", json!({"sites": ["example.com"]}), json!([hours.clone()])),
        block("app", json!({"apps": [APP]}), json!([session("s", "2026-11-05T11:00:00Z")])),
    ]);
    check("the listed host", &sites, Target::Host("example.com"), &nobody, "UTC", ten, seventeen);
    check("a subdomain", &sites, Target::Host("www.example.com"), &nobody, "UTC", ten, seventeen);
    check("another host", &sites, Target::Host("example.org"), &nobody, "UTC", ten, None);
    check("the app, whose own block ends at 11:00", &sites, Target::App(APP), &nobody, "UTC", ten, Some(moment("2026-11-05T11:00:00Z")));
    check("an app named like the host", &sites, Target::App("example.com"), &nobody, "UTC", ten, None);

    // An app that is never blocked has no flip, though the same block flips for every other app.
    let all_but_one = read_set(&[block("b", json!({"allAppsExcept": ["com.keep"]}), json!([hours.clone()]))]);
    let guard: HashSet<String> = ["com.guard".to_string()].into();
    check("an app the block lets through", &all_but_one, Target::App("com.keep"), &nobody, "UTC", ten, None);
    check("another app", &all_but_one, Target::App("com.other"), &nobody, "UTC", ten, seventeen);
    check("a never-blocked app", &all_but_one, Target::App("com.guard"), &guard, "UTC", ten, None);
    check("the same app when it is not on the list", &all_but_one, Target::App("com.guard"), &nobody, "UTC", ten, seventeen);

    // Rules that count use are never a flip, beside other rules or alone.
    let counting_beside = read_set(&[block("b", json!({"apps": [APP]}), json!([minutes("m"), hours.clone(), opens("o")]))]);
    check("rules that count use beside hours", &counting_beside, Target::App(APP), &nobody, "UTC", ten, seventeen);
    let counting_alone = read_set(&[block("b", json!({"apps": [APP]}), json!([minutes("m"), opens("o")]))]);
    check("rules that count use alone", &counting_alone, Target::App(APP), &nobody, "UTC", ten, None);

    // Nothing to wake up for.
    let off_only = read_set(&[block_with("off", json!({"apps": [APP]}), json!([hours.clone()]), json!({"on": false}))]);
    let quiet = [
        ("no blocks", ReadSet::default()),
        ("only a switched-off block", off_only),
        ("an always rule on every day", one_rule(always("r", &[]))),
        ("outside hours with no stretch", one_rule(outside_hours("r", &[], &[]))),
        ("a stretch of the whole day", one_rule(outside_hours("r", &[], &[("00:00", "24:00")]))),
        ("only another app's block", read_set(&[block("b", json!({"apps": ["com.other"]}), json!([hours]))])),
    ];
    for (what, set) in &quiet {
        expect_app(what, set, NEW_YORK, moment("2026-11-05T10:00:00-05:00"), None);
    }
}

#[test]
fn the_largest_moments_the_core_can_count_have_no_flip_and_do_not_overflow() {
    let busy = read_set(&[block("b", json!({"apps": [APP]}), json!([always("m", &["mon"]), outside_hours("h", &[], &[("09:00", "17:00")]), session("s", "2026-11-05T18:30:00Z")]))]);
    for extreme in [i64::MAX, i64::MAX - 1, i64::MAX - 1000, i64::MAX - 8 * DAY_MS + 1] {
        assert_eq!(next_flip(&busy, Target::App(APP), &none_blocked(), extreme, &zone(NEW_YORK)), None, "from {extreme}");
    }
    // The smallest moment is where no zone rule exists: any answer must still be later than the
    // question and within the 8 days.
    if let Some(at) = next_flip(&busy, Target::App(APP), &none_blocked(), i64::MIN, &zone(NEW_YORK)) {
        assert!(at > i64::MIN && at <= i64::MIN + 8 * DAY_MS, "from the smallest moment: {at}");
    }
}
