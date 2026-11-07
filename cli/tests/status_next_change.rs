//! The sentence says until when its answer holds, in the local time of the zone it was judged in
//! (AC-007): the next moment this app or site flips between closed and open, not the next edge of any
//! rule in the file. Every sentence is compared whole.

mod run_capture;
mod scratch_folder;
mod settings_builder;

use run_capture::{status, Outcome};
use scratch_folder::Scratch;
use serde_json::{json, Value};
use settings_builder::{always, block, block_with, outside_hours, session, write_settings};
use std::path::{Path, PathBuf};

const NOW: &str = "2026-11-05T15:00:00Z";
const NEVER_BLOCKED: &str = "Apps that are never blocked are not applied yet. Nothing was saved or changed.\n";

fn settings(blocks: &[Value]) -> (Scratch, PathBuf) {
    let folder = Scratch::new("until");
    let file = write_settings(folder.path(), "settings.zip", blocks);
    (folder, file)
}

/// The one sentence the command writes about `app` at `now`, judged in `zone`.
fn app_sentence(file: &Path, app: &str, zone: &str, now: &str) -> String {
    let run: Outcome = status(file, &["--app", app, "--zone", zone], now, None);
    assert_eq!((run.code, run.err.as_str()), (0, ""), "{app}: {}", run.err);
    assert_eq!(run.out.matches('\n').count(), 1, "{app}: {:?}", run.out);
    run.out
}

fn closed_by(rule_text: &str) -> String {
    format!("Closed by {rule_text}.")
}

#[test]
fn the_sentence_says_until_when_the_answer_holds_in_local_time() {
    let (_folder, file) = settings(&[
        block("ended", json!({"apps": ["com.ended"]}), json!([session("x", "2026-11-05T14:59:59Z")])),
        block("exact", json!({"apps": ["com.exact"]}), json!([session("x", NOW)])),
        block("soon", json!({"apps": ["com.soon"]}), json!([session("x", "2026-11-07T15:00:00Z")])),
        block("week", json!({"apps": ["com.week"]}), json!([session("x", "2026-11-12T15:00:00Z")])),
        block("edge", json!({"apps": ["com.edge"]}), json!([session("x", "2026-11-13T15:00:00Z")])),
        block("far", json!({"apps": ["com.far"]}), json!([session("x", "2026-11-13T15:01:00Z")])),
    ]);
    let ny = "America/New_York";
    let judged = "judged at 2026-11-05T10:00:00-05:00 in America/New_York.";
    let said = |app: &str, state: &str, span: &str, rest: &str| format!("App \"{app}\" is {state} {span}, {judged} {rest}{NEVER_BLOCKED}");

    // A session that ended a second ago, or ends this very moment, no longer closes anything.
    assert_eq!(app_sentence(&file, "com.ended", ny, NOW), said("com.ended", "open", "for at least the next 8 days", ""));
    assert_eq!(app_sentence(&file, "com.exact", ny, NOW), said("com.exact", "open", "for at least the next 8 days", ""));

    // One that ends later closes until then: two days, seven days, and exactly eight days ahead; one
    // minute beyond the look-ahead is not seen, and the sentence says how far it did look.
    for (app, until) in [("soon", "2026-11-07T10:00:00-05:00"), ("week", "2026-11-12T10:00:00-05:00"), ("edge", "2026-11-13T10:00:00-05:00")] {
        let rest = format!("{} ", closed_by(&format!("block \"{app}\" rule \"x\" (session)")));
        assert_eq!(app_sentence(&file, &format!("com.{app}"), ny, NOW), said(&format!("com.{app}"), "closed", &format!("until {until}"), &rest), "{app}");
    }
    let far = closed_by("block \"far\" rule \"x\" (session)");
    assert_eq!(app_sentence(&file, "com.far", ny, NOW), said("com.far", "closed", "for at least the next 8 days", &format!("{far} ")));

    // The moment is written in the zone's own offset: half hours, a negative half hour, and none.
    let soon = closed_by("block \"soon\" rule \"x\" (session)");
    for (zone, local_now, until) in [
        ("Asia/Kolkata", "2026-11-05T20:30:00+05:30", "2026-11-07T20:30:00+05:30"),
        ("America/St_Johns", "2026-11-05T11:30:00-03:30", "2026-11-07T11:30:00-03:30"),
        ("UTC", "2026-11-05T15:00:00Z", "2026-11-07T15:00:00Z"),
        ("Asia/Tokyo", "2026-11-06T00:00:00+09:00", "2026-11-08T00:00:00+09:00"),
    ] {
        let expected = format!("App \"com.soon\" is closed until {until}, judged at {local_now} in {zone}. {soon} {NEVER_BLOCKED}");
        assert_eq!(app_sentence(&file, "com.soon", zone, NOW), expected, "{zone}");
    }
}

#[test]
fn the_flip_is_the_target_s_own_and_lands_on_the_local_clock_even_when_the_clock_repeats_or_skips() {
    let (_folder, file) = settings(&[
        block("dst", json!({"apps": ["com.d"]}), json!([outside_hours("r", &[], &[("01:30", "03:00")])])),
        block("spring", json!({"apps": ["com.s"]}), json!([outside_hours("r", &[], &[("02:30", "04:00")])])),
        block("friday", json!({"apps": ["com.f"]}), json!([always("f", &["fri"])])),
    ]);
    let ny = "America/New_York";
    let say = |app: &str, state: &str, until: &str, local_now: &str, rest: &str| format!("App \"{app}\" is {state} until {until}, judged at {local_now} in America/New_York. {rest}{NEVER_BLOCKED}");
    let outside = closed_by("block \"dst\" rule \"r\" (outsideHours)") + " ";

    // The clocks go back at 2026-11-01T06:00Z: 01:00 to 02:00 happens twice. The same wall time has a
    // different offset the second time, and the flip is written with the offset it really has.
    assert_eq!(app_sentence(&file, "com.d", ny, "2026-11-01T04:30:00Z"), say("com.d", "closed", "2026-11-01T01:30:00-04:00", "2026-11-01T00:30:00-04:00", &outside));
    assert_eq!(app_sentence(&file, "com.d", ny, "2026-11-01T05:45:00Z"), say("com.d", "open", "2026-11-01T01:00:00-05:00", "2026-11-01T01:45:00-04:00", ""));
    assert_eq!(app_sentence(&file, "com.d", ny, "2026-11-01T06:00:00Z"), say("com.d", "closed", "2026-11-01T01:30:00-05:00", "2026-11-01T01:00:00-05:00", &outside));
    assert_eq!(app_sentence(&file, "com.d", ny, "2026-11-01T06:30:00Z"), say("com.d", "open", "2026-11-01T03:00:00-05:00", "2026-11-01T01:30:00-05:00", ""));

    // The clocks go forward at 2026-03-08T07:00Z: 02:30 does not exist that day, so the rule opens at
    // the first moment that is past it, 03:00 summer time.
    let spring = closed_by("block \"spring\" rule \"r\" (outsideHours)") + " ";
    assert_eq!(app_sentence(&file, "com.s", ny, "2026-03-08T06:30:00Z"), say("com.s", "closed", "2026-03-08T03:00:00-04:00", "2026-03-08T01:30:00-05:00", &spring));

    // A rule on one weekday flips at local midnight: Thursday evening is open until Friday begins, and
    // Friday is closed until Saturday begins.
    let friday = closed_by("block \"friday\" rule \"f\" (always)") + " ";
    assert_eq!(app_sentence(&file, "com.f", ny, NOW), say("com.f", "open", "2026-11-06T00:00:00-05:00", "2026-11-05T10:00:00-05:00", ""));
    assert_eq!(app_sentence(&file, "com.f", ny, "2026-11-06T17:00:00Z"), say("com.f", "closed", "2026-11-07T00:00:00-05:00", "2026-11-06T12:00:00-05:00", &friday));
}

#[test]
fn edges_of_other_blocks_of_other_targets_and_of_rules_under_a_closing_rule_are_not_flips() {
    let (_folder, file) = settings(&[
        block("mine", json!({"apps": ["com.a"]}), json!([outside_hours("r", &[], &[("09:00", "17:00")])])),
        block("other", json!({"apps": ["com.b"]}), json!([session("x", "2026-11-05T16:00:00Z")])),
        block_with("off", json!({"apps": ["com.a"]}), json!([session("x", "2026-11-05T16:30:00Z")]), json!({"on": false})),
        block("web", json!({"sites": ["example.com"]}), json!([session("x", "2026-11-05T16:00:00Z")])),
        block("under", json!({"apps": ["com.u"]}), json!([always("u1", &[]), session("u2", "2026-11-06T00:00:00Z")])),
        block("both", json!({"apps": ["com.v"]}), json!([outside_hours("h", &[], &[("09:00", "17:00")]), session("s", "2026-11-07T15:00:00Z")])),
    ]);
    let ny = "America/New_York";
    let judged = "judged at 2026-11-05T10:00:00-05:00 in America/New_York.";

    // Other blocks and a switched-off block have edges soon, but this app's own next edge is at 17:00.
    let mine = format!("App \"com.a\" is open until 2026-11-05T17:00:00-05:00, {judged} {NEVER_BLOCKED}");
    assert_eq!(app_sentence(&file, "com.a", ny, NOW), mine);

    // The site's own session is its edge, and a site's sentence has no note about apps.
    let site = status(&file, &["--site", "example.com", "--zone", ny], NOW, None);
    let site_text = format!("Site \"example.com\" is closed until 2026-11-05T11:00:00-05:00, {judged} Closed by block \"web\" rule \"x\" (session). Nothing was saved or changed.\n");
    assert_eq!((site.code, site.err.as_str(), site.out.as_str()), (0, "", site_text.as_str()));

    // A session that ends under a rule that still closes does not open anything: no flip is seen.
    let under = closed_by("block \"under\" rule \"u1\" (always); block \"under\" rule \"u2\" (session)");
    assert_eq!(app_sentence(&file, "com.u", ny, NOW), format!("App \"com.u\" is closed for at least the next 8 days, {judged} {under} {NEVER_BLOCKED}"));

    // Two rules close in turn and never leave a gap: the answer stays closed until the later one ends,
    // not until the hours rule's edges (17:00, 09:00, 17:00 ...).
    let both = closed_by("block \"both\" rule \"s\" (session)");
    assert_eq!(app_sentence(&file, "com.v", ny, NOW), format!("App \"com.v\" is closed until 2026-11-07T10:00:00-05:00, {judged} {both} {NEVER_BLOCKED}"));
}
