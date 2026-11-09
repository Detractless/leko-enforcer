//! The next change is the moment a caller wakes up and asks again: the first instant after the given
//! one at which a rule of an on block opens or closes (AC-009). Every answer is also checked against
//! the decision itself: nothing differs at the moments sampled before it, and something differs at it.

mod block_builder;
mod zip_writer;

use block_builder::{always, block, block_with, minutes, moment, none_blocked, one_rule, opens, outside_hours, read_set, session, zone, APP, DAY_MS, NEW_YORK};
use leko_core::decision::block_cover::Target;
use leko_core::decision::decide_target::{decide, RuleRef};
use leko_core::decision::next_change::next_change;
use leko_core::models::specs::ReadSet;
use serde_json::json;

fn closing_at(set: &ReadSet, zone_name: &str, at_ms: i64) -> Vec<RuleRef> {
    decide(set, Target::App(APP), at_ms, &zone(zone_name), &none_blocked()).closing
}

/// Asks for the next change after `from` and checks it is `expected`, then asks the decision itself:
/// every 20 seconds before the answer (every 5 minutes for no answer) the closing rules are those of
/// `from`, one millisecond before the answer they still are, and at the answer they differ.
fn expect_change(what: &str, set: &ReadSet, zone_name: &str, from: i64, expected: Option<i64>) {
    let answer = next_change(set, from, &zone(zone_name));
    assert_eq!(answer, expected, "{what}");
    assert!(answer.is_none_or(|at| at > from), "{what}: the answer is not after the question");
    let start = closing_at(set, zone_name, from);
    let (end, step) = match expected {
        Some(at) => (at, 20_000),
        None => (from + 8 * DAY_MS, 300_000),
    };
    let mut sample = from + step;
    while sample < end {
        assert_eq!(closing_at(set, zone_name, sample), start, "{what}: the closing rules changed before the answer, at {sample}");
        sample += step;
    }
    if let Some(at) = expected {
        assert_eq!(closing_at(set, zone_name, at - 1), start, "{what}: the closing rules changed within a millisecond before the answer");
        assert_ne!(closing_at(set, zone_name, at), start, "{what}: nothing differs at the answer");
    }
}

#[test]
fn the_next_change_is_the_first_moment_the_closing_state_differs() {
    // A stretch edge is that minute, however far into the minute the question comes; at the edge itself
    // the state is already the new one, so the answer is the change after it.
    let office = one_rule(outside_hours("r", &[], &[("09:00", "17:00")]));
    let five = Some(moment("2026-11-05T17:00:00-05:00"));
    let nine = Some(moment("2026-11-06T09:00:00-05:00"));
    expect_change("hours, from 10:00", &office, NEW_YORK, moment("2026-11-05T10:00:00-05:00"), five);
    expect_change("hours, from 10:00:30", &office, NEW_YORK, moment("2026-11-05T10:00:30-05:00"), five);
    expect_change("hours, from the last millisecond", &office, NEW_YORK, moment("2026-11-05T16:59:59.999-05:00"), five);
    expect_change("hours, from the edge", &office, NEW_YORK, moment("2026-11-05T17:00:00-05:00"), nine);
    expect_change("hours, from 18:00", &office, NEW_YORK, moment("2026-11-05T18:00:00-05:00"), nine);

    // A rule on one weekday: the next day boundary, on the zone's calendar.
    let mondays = one_rule(always("r", &["mon"]));
    expect_change("Monday rule, from Tuesday", &mondays, NEW_YORK, moment("2026-11-03T12:00:00-05:00"), Some(moment("2026-11-09T00:00:00-05:00")));
    expect_change("Monday rule, from Monday", &mondays, NEW_YORK, moment("2026-11-09T12:00:00-05:00"), Some(moment("2026-11-10T00:00:00-05:00")));

    // Seven days and an hour away: the week with the repeated hour of autumn. Closed only on Monday
    // 23:59; asked on Tuesday 00:00:30 before the clocks go back, the answer is more than 7 days ahead.
    let last_minute = one_rule(outside_hours("r", &["mon"], &[("00:00", "23:59")]));
    let from = moment("2026-10-27T00:00:30-04:00");
    let answer = moment("2026-11-02T23:59:00-05:00");
    assert!(answer - from > 7 * DAY_MS, "the case must reach past a week");
    expect_change("a week and an hour away", &last_minute, NEW_YORK, from, Some(answer));

    // A session end is that second, and is found only within 8 days of the question.
    let end = moment("2026-11-13T10:00:00Z");
    let ends = one_rule(session("s", "2026-11-13T10:00:00Z"));
    expect_change("session end, exactly 8 days ahead", &ends, "UTC", end - 8 * DAY_MS, Some(end));
    expect_change("session end, just under 8 days ahead", &ends, "UTC", end - 8 * DAY_MS + 1, Some(end));
    expect_change("session end, 8 days and a millisecond ahead", &ends, "UTC", end - 8 * DAY_MS - 1, None);
    expect_change("session end, 9 days ahead", &ends, "UTC", end - 9 * DAY_MS, None);
    expect_change("session end, asked at the end", &ends, "UTC", end, None);
    expect_change("session end, asked after the end", &ends, "UTC", end + DAY_MS, None);
    let seconds = one_rule(session("s", "2026-11-05T18:30:01Z"));
    expect_change("session end between two minutes", &seconds, NEW_YORK, moment("2026-11-05T18:00:00Z"), Some(moment("2026-11-05T18:30:01Z")));
    expect_change("asked in the minute the session ended", &seconds, NEW_YORK, moment("2026-11-05T18:30:30Z"), None);
    // Two ends between minutes: the earlier one is the answer, not the later one.
    let two_ends = read_set(&[
        block("early", json!({"apps": [APP]}), json!([session("s", "2026-11-05T18:30:01Z")])),
        block("late", json!({"apps": [APP]}), json!([session("s", "2026-11-05T18:45:01Z")])),
    ]);
    expect_change("two session ends between minutes", &two_ends, NEW_YORK, moment("2026-11-05T18:00:00Z"), Some(moment("2026-11-05T18:30:01Z")));
    // An end that is not on a whole minute, exactly 8 days ahead of a question that is not either.
    let off_beat = moment("2026-11-13T10:00:01Z");
    let off_beat_rule = one_rule(session("s", "2026-11-13T10:00:01Z"));
    expect_change("session end 8 days ahead, between minutes", &off_beat_rule, "UTC", off_beat - 8 * DAY_MS, Some(off_beat));
    expect_change("session end 8 days and a millisecond ahead, between minutes", &off_beat_rule, "UTC", off_beat - 8 * DAY_MS - 1, None);

    // The earliest of several blocks wins, and a switched-off block is not looked at.
    let mixed = read_set(&[
        block("hours", json!({"apps": [APP]}), json!([outside_hours("r", &[], &[("09:00", "17:00")])])),
        block("short", json!({"apps": [APP]}), json!([session("s", "2026-11-05T12:00:00Z")])),
        block_with("off", json!({"apps": [APP]}), json!([outside_hours("r", &[], &[("10:30", "11:00")])]), json!({"on": false})),
    ]);
    expect_change("two blocks and an off block", &mixed, "UTC", moment("2026-11-05T10:00:00Z"), Some(moment("2026-11-05T12:00:00Z")));

    // Clock changes: the spring jump is the change, and the repeated autumn hour is judged by the wall clock both times.
    let late_start = one_rule(outside_hours("r", &[], &[("02:30", "04:00")]));
    expect_change("spring jump", &late_start, NEW_YORK, moment("2026-03-08T06:59:00Z"), Some(moment("2026-03-08T07:00:00Z")));
    let quarter = one_rule(outside_hours("r", &[], &[("01:15", "01:45")]));
    expect_change("autumn, first pass", &quarter, NEW_YORK, moment("2026-11-01T05:40:00Z"), Some(moment("2026-11-01T05:45:00Z")));
    expect_change("autumn, between the passes", &quarter, NEW_YORK, moment("2026-11-01T05:50:00Z"), Some(moment("2026-11-01T06:15:00Z")));

    // Nothing to wake up for.
    let off_only = read_set(&[block_with("off", json!({"apps": [APP]}), json!([always("r", &["mon"])]), json!({"on": false}))]);
    let counting = read_set(&[block("b", json!({"apps": [APP]}), json!([minutes("m"), opens("o")]))]);
    let quiet = [
        ("no blocks", ReadSet::default()),
        ("only a switched-off block", off_only),
        ("only rules that count use", counting),
        ("an always rule on every day", one_rule(always("r", &[]))),
        ("outside hours with no stretch", one_rule(outside_hours("r", &[], &[]))),
        ("a stretch of the whole day", one_rule(outside_hours("r", &[], &[("00:00", "24:00")]))),
    ];
    for (what, set) in &quiet {
        expect_change(what, set, NEW_YORK, moment("2026-11-05T10:00:00-05:00"), None);
    }

    // The largest moments the core can count: no answer and no overflow.
    let busy = read_set(&[block("b", json!({"apps": [APP]}), json!([always("m", &["mon"]), outside_hours("h", &[], &[("09:00", "17:00")]), session("s", "2026-11-05T18:30:00Z")]))]);
    for extreme in [i64::MAX, i64::MAX - 1, i64::MAX - 1000, i64::MAX - 8 * DAY_MS + 1] {
        assert_eq!(next_change(&busy, extreme, &zone(NEW_YORK)), None, "from {extreme}");
    }
    // The smallest moment is year -292 million, where no zone rule exists: any answer must still be
    // later than the question and within the 8 days.
    if let Some(at) = next_change(&busy, i64::MIN, &zone(NEW_YORK)) {
        assert!(at > i64::MIN && at <= i64::MIN + 8 * DAY_MS, "from the smallest moment: {at}");
    }
}
