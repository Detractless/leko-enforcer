//! `leko status FILE --app NAME | --site HOST` says whether one app or site is closed or open at the
//! moment it is given, naming every closing rule (AC-001), and names the rules that count use as not
//! judged, never as open or closed (AC-002). Every sentence is compared whole.

mod status_support;
use status_support::*;

use run_capture::{json_of, status, Outcome};
use serde_json::json;
use settings_builder::{always, block, block_with, minutes, opens, outside_hours, session};

const NOON: &str = "2026-11-05T15:00:00Z";
const ZONE: [&str; 2] = ["--zone", "America/New_York"];
const NEVER_BLOCKED: &str = "Apps that are never blocked are not applied yet.";
const TAIL: &str = "Nothing was saved or changed.\n";

fn ask(file: &std::path::Path, target: [&str; 2], now: &str) -> Outcome {
    let flags = [&target[..], &ZONE[..]].concat();
    status(file, &flags, now, None)
}

#[test]
fn an_app_or_a_site_is_reported_closed_with_its_closing_rules_or_open_when_none_closes() {
    let (_folder, file) = settings("status", &[
        block("zeta", json!({"apps": ["com.a"], "sites": ["example.com"]}), json!([always("z1", &[]), session("z2", "2026-11-05T20:00:00Z")])),
        block("alpha", json!({"apps": ["com.a"]}), json!([always("a1", &[])])),
        block("elsewhere", json!({"apps": ["com.b"], "sites": ["other.org"]}), json!([always("e1", &[])])),
        block_with("off", json!({"apps": ["com.a"], "sites": ["example.com"]}), json!([always("o1", &[])]), json!({"on": false})),
        block("hours", json!({"apps": ["com.h"]}), json!([outside_hours("r", &[], &[("09:00", "17:00")])])),
    ]);
    let judged = "judged at 2026-11-05T10:00:00-05:00 in America/New_York.";

    // An app: every closing rule, blocks in the order of their ids, rules as the file writes them; a
    // switched-off block and another app's block are left out.
    let app = ask(&file, ["--app", "com.a"], NOON);
    let closers = r#"Closed by block "alpha" rule "a1" (always); block "zeta" rule "z1" (always); block "zeta" rule "z2" (session)."#;
    assert_eq!(answered(&app), format!("App \"com.a\" is closed for at least the next 8 days, {judged} {closers} {NEVER_BLOCKED} {TAIL}"));
    let other = ask(&file, ["--app", "com.b"], NOON);
    assert_eq!(answered(&other), format!("App \"com.b\" is closed for at least the next 8 days, {judged} Closed by block \"elsewhere\" rule \"e1\" (always). {NEVER_BLOCKED} {TAIL}"));

    // A site is judged by the sites of the blocks, a subdomain by the name it is under, and the
    // sentence has no note about apps that are never blocked.
    let site = ask(&file, ["--site", "example.com"], NOON);
    let site_closers = r#"Closed by block "zeta" rule "z1" (always); block "zeta" rule "z2" (session)."#;
    assert_eq!(answered(&site), format!("Site \"example.com\" is closed for at least the next 8 days, {judged} {site_closers} {TAIL}"));
    let subdomain = ask(&file, ["--site", "www.example.com"], NOON);
    assert_eq!(answered(&subdomain), format!("Site \"www.example.com\" is closed for at least the next 8 days, {judged} {site_closers} {TAIL}"));
    let other_site = ask(&file, ["--site", "other.org"], NOON);
    assert_eq!(answered(&other_site), format!("Site \"other.org\" is closed for at least the next 8 days, {judged} Closed by block \"elsewhere\" rule \"e1\" (always). {TAIL}"));

    // Open is an answer too (exit 0): nothing closes it. An app is not a site, and a site is not an app.
    for (target, kind) in [(["--app", "com.zzz"], "App"), (["--app", "example.com"], "App"), (["--site", "com.a"], "Site"), (["--site", "elsewhere.net"], "Site")] {
        let run = ask(&file, target, NOON);
        let note = if kind == "App" { format!("{NEVER_BLOCKED} ") } else { String::new() };
        assert_eq!(answered(&run), format!("{kind} \"{}\" is open for at least the next 8 days, {judged} {note}{TAIL}", target[1]), "{target:?}");
    }

    // The sentence is judged at the moment handed in: the same file, two moments.
    let morning = ask(&file, ["--app", "com.h"], NOON);
    assert_eq!(answered(&morning), format!("App \"com.h\" is open until 2026-11-05T17:00:00-05:00, {judged} {NEVER_BLOCKED} {TAIL}"));
    let evening = ask(&file, ["--app", "com.h"], "2026-11-05T23:30:00Z");
    let judged_evening = "judged at 2026-11-05T18:30:00-05:00 in America/New_York.";
    assert_eq!(answered(&evening), format!("App \"com.h\" is closed until 2026-11-06T09:00:00-05:00, {judged_evening} Closed by block \"hours\" rule \"r\" (outsideHours). {NEVER_BLOCKED} {TAIL}"));
    assert_eq!(ask(&file, ["--app", "com.h"], NOON).out, morning.out, "the same inputs give the same sentence");

    // A settings file with no blocks, with only switched-off blocks, or with none naming the target.
    let (_empty_folder, empty) = settings("status", &[]);
    assert_eq!(answered(&ask(&empty, ["--app", "com.a"], NOON)), format!("App \"com.a\" is open for at least the next 8 days, {judged} {NEVER_BLOCKED} {TAIL}"));
    let (_off_folder, off) = settings("status", &[block_with("off", json!({"apps": ["com.a"]}), json!([always("o1", &[])]), json!({"on": false}))]);
    assert_eq!(answered(&ask(&off, ["--app", "com.a"], NOON)), format!("App \"com.a\" is open for at least the next 8 days, {judged} {NEVER_BLOCKED} {TAIL}"));
}

#[test]
fn rules_that_count_use_are_named_as_not_judged_and_never_called_closed_or_checked() {
    let (_folder, file) = settings("status", &[
        block("limits", json!({"apps": ["com.m"]}), json!([minutes("m1"), opens("o1")])),
        block("mixed", json!({"apps": ["com.x"]}), json!([minutes("m2"), outside_hours("h", &[], &[("09:00", "17:00")])])),
    ]);
    let judged = "judged at 2026-11-05T10:00:00-05:00 in America/New_York.";
    let not_judged = "Not judged, because they count use and no use is counted yet:";

    // Covered only by rules that count use: open, with those rules named.
    let only = ask(&file, ["--app", "com.m"], NOON);
    assert_eq!(answered(&only), format!("App \"com.m\" is open for at least the next 8 days, {judged} {not_judged} block \"limits\" rule \"m1\" (minutes); block \"limits\" rule \"o1\" (opens). {NEVER_BLOCKED} {TAIL}"));
    let json = json_of(&status(&file, &["--app", "com.m", "--json", "--zone", "America/New_York"], NOON, None));
    assert_eq!(json["closed"], json!(false));
    assert_eq!(json["closing"], json!([]));
    assert_eq!(json["not_judged"], json!([{"block": "limits", "rule": "m1", "kind": "minutes"}, {"block": "limits", "rule": "o1", "kind": "opens"}]));

    // Beside a rule that is judged: the judged rule decides, the other is still named.
    let open = ask(&file, ["--app", "com.x"], NOON);
    assert_eq!(answered(&open), format!("App \"com.x\" is open until 2026-11-05T17:00:00-05:00, {judged} {not_judged} block \"mixed\" rule \"m2\" (minutes). {NEVER_BLOCKED} {TAIL}"));
    let closed = ask(&file, ["--app", "com.x"], "2026-11-05T23:30:00Z");
    let evening = "judged at 2026-11-05T18:30:00-05:00 in America/New_York.";
    assert_eq!(answered(&closed), format!("App \"com.x\" is closed until 2026-11-06T09:00:00-05:00, {evening} Closed by block \"mixed\" rule \"h\" (outsideHours). {not_judged} block \"mixed\" rule \"m2\" (minutes). {NEVER_BLOCKED} {TAIL}"));

    // Nothing the sentence says about a rule that counts use is a verdict.
    for run in [&only, &open, &closed] {
        let text = &run.out;
        assert!(!text.contains("m1\" (minutes) is") && !text.contains("satisf") && !text.contains("checked") && !text.contains("free"), "{text}");
    }
}
