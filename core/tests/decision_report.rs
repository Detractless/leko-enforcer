//! The answer says why: every closing block and rule, blocks in the order of their ids and rules as
//! written (AC-007), and rules that count use are named as not judged, never taken for open (AC-008).

mod block_builder;
mod zip_writer;

use block_builder::{always, block, minutes, moment, none_blocked, opens, outside_hours, read_set, session, zone};
use leko_core::decision::block_cover::Target;
use leko_core::decision::decide_target::{decide, Decision, RuleRef};
use leko_core::models::specs::ReadSet;
use serde_json::{json, Value};

const NOW: &str = "2026-11-05T10:00:00Z";

fn ask(set: &ReadSet, app: &str) -> Decision {
    decide(set, Target::App(app), moment(NOW), &zone("UTC"), &none_blocked())
}

fn names(rules: &[RuleRef]) -> Vec<String> {
    rules.iter().map(|rule| format!("{}/{}/{}", rule.block_id, rule.rule_id, rule.kind)).collect()
}

fn blocks_for_one_app() -> Vec<Value> {
    // 2026-11-05 is a Thursday. "open_today" does not close today; the others do.
    vec![
        block("zeta", json!({"apps": ["com.a"]}), json!([session("z1", "2026-11-06T00:00:00Z")])),
        block("alpha", json!({"apps": ["com.a"]}), json!([always("second", &[]), always("first", &[]), always("open_today", &["mon"])])),
        block("mid", json!({"allAppsExcept": ["com.zzz"]}), json!([always("not_today", &["sun"]), outside_hours("hours", &[], &[("20:00", "21:00")])])),
        block("other", json!({"apps": ["com.other"]}), json!([always("x", &[])])),
    ]
}

#[test]
fn the_answer_names_every_closing_block_and_rule_in_a_fixed_order() {
    let blocks = blocks_for_one_app();
    let answer = ask(&read_set(&blocks), "com.a");
    assert!(answer.closed);
    assert_eq!(
        names(&answer.closing),
        ["alpha/second/always", "alpha/first/always", "mid/hours/outsideHours", "zeta/z1/session"],
        "blocks by id, rules as written; a rule that is open today is not listed, and a block that does not cover the app is not listed"
    );
    assert!(answer.not_judged.is_empty());

    // The same answer whatever order the files were read in, and when asked again.
    let mut reversed = blocks.clone();
    reversed.reverse();
    assert_eq!(ask(&read_set(&reversed), "com.a"), answer, "the order of the files does not matter");
    assert_eq!(ask(&read_set(&blocks), "com.a"), answer, "asking twice gives the same answer");
    // The reader may hand the blocks over in any order; the answer must not depend on it.
    for rotation in 0..blocks.len() {
        let mut turned = read_set(&blocks);
        turned.blocks.items.rotate_left(rotation);
        assert_eq!(ask(&turned, "com.a"), answer, "the blocks turned by {rotation}");
        turned.blocks.items.reverse();
        assert_eq!(ask(&turned, "com.a"), answer, "the blocks turned by {rotation}, then reversed");
    }

    // `closed` is true exactly when something is listed.
    let nothing = ask(&read_set(&blocks), "com.zzz");
    assert!(!nothing.closed && nothing.closing.is_empty(), "{nothing:?}");
    for app in ["com.a", "com.other", "com.zzz", "com.unknown"] {
        let decision = ask(&read_set(&blocks), app);
        assert_eq!(decision.closed, !decision.closing.is_empty(), "{app}");
    }
}

#[test]
fn minutes_and_opens_rules_are_reported_as_not_judged_never_as_open() {
    let set = read_set(&[
        block("counting", json!({"apps": ["com.m"]}), json!([minutes("m1"), opens("o1")])),
        block("mixed", json!({"apps": ["com.x"]}), json!([always("r1", &[]), minutes("m2")])),
        block("quiet", json!({"apps": ["com.q"]}), json!([always("r1", &["mon"]), minutes("m3")])),
    ]);

    // Covered only by rules that count use: open, and the rules are named.
    let counting = ask(&set, "com.m");
    assert!(!counting.closed && counting.closing.is_empty(), "{counting:?}");
    assert_eq!(names(&counting.not_judged), ["counting/m1/minutes", "counting/o1/opens"]);

    // A time rule that closes and a rule that counts use: both lists are filled.
    let mixed = ask(&set, "com.x");
    assert!(mixed.closed);
    assert_eq!(names(&mixed.closing), ["mixed/r1/always"]);
    assert_eq!(names(&mixed.not_judged), ["mixed/m2/minutes"]);

    // The time rule is open today (Thursday) but the rule that counts use is still named.
    let quiet = ask(&set, "com.q");
    assert!(!quiet.closed);
    assert_eq!(names(&quiet.not_judged), ["quiet/m3/minutes"]);

    // Not covered, or switched off: nothing is named.
    assert!(ask(&set, "com.nobody").not_judged.is_empty());
    let off = read_set(&[block_builder::block_with("off", json!({"apps": ["com.o"]}), json!([minutes("m")]), json!({"on": false}))]);
    assert!(ask(&off, "com.o").not_judged.is_empty(), "a switched-off block is not mentioned");
}
