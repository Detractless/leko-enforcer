//! What a block covers decides what it can close: a switched-off block closes nothing and a missing
//! switch means on (AC-001); named apps, all-except apps and sites each cover only their own, and an
//! app that is never blocked stays open under every block, a session included (AC-006).

mod block_builder;
mod zip_writer;

use block_builder::{always, block, block_with, minutes, moment, none_blocked, outside_hours, read_set, session, zone};
use leko_core::decision::block_cover::Target;
use leko_core::decision::decide_target::{decide, Decision};
use leko_core::models::specs::ReadSet;
use serde_json::json;
use std::collections::HashSet;

const NOW: &str = "2026-11-05T10:00:00Z";

fn ask(set: &ReadSet, target: Target, never_blocked: &HashSet<String>) -> Decision {
    decide(set, target, moment(NOW), &zone("UTC"), never_blocked)
}

fn closers(decision: &Decision) -> Vec<String> {
    decision.closing.iter().map(|rule| format!("{}/{}", rule.block_id, rule.rule_id)).collect()
}

#[test]
fn an_off_block_closes_nothing_and_a_missing_on_means_on() {
    let set = read_set(&[
        block("plain", json!({"apps": ["com.plain"]}), json!([always("a", &[])])),
        block_with("explicit", json!({"apps": ["com.explicit"]}), json!([always("a", &[])]), json!({"on": true})),
        block_with("off", json!({"apps": ["com.off"]}), json!([always("a", &[]), minutes("m")]), json!({"on": false})),
    ]);
    let nobody = none_blocked();

    let plain = ask(&set, Target::App("com.plain"), &nobody);
    assert!(plain.closed, "a block with no `on` field is on");
    assert_eq!(closers(&plain), ["plain/a"]);
    assert!(ask(&set, Target::App("com.explicit"), &nobody).closed, "`on: true` is on");

    let off = ask(&set, Target::App("com.off"), &nobody);
    assert!(!off.closed, "a switched-off block closes nothing");
    assert!(off.closing.is_empty(), "{off:?}");
    assert!(off.not_judged.is_empty(), "a switched-off block is not mentioned at all: {off:?}");

    let unknown = ask(&set, Target::App("com.nobody.names.this"), &nobody);
    assert_eq!(unknown, Decision { closed: false, closing: Vec::new(), not_judged: Vec::new() });

    // No blocks at all, and only a switched-off block, answer open.
    let empty = ask(&ReadSet::default(), Target::App("com.plain"), &nobody);
    assert_eq!(empty, Decision { closed: false, closing: Vec::new(), not_judged: Vec::new() });
    let only_off = read_set(&[block_with("off", json!({"apps": ["com.off"]}), json!([always("a", &[])]), json!({"on": false}))]);
    assert!(!ask(&only_off, Target::App("com.off"), &nobody).closed);
}

#[test]
fn named_apps_all_except_and_sites_decide_what_a_block_covers() {
    let far_end = "2027-01-01T00:00:00Z";
    let set = read_set(&[
        block("named", json!({"apps": ["com.named"]}), json!([always("a", &[])])),
        block("except", json!({"allAppsExcept": ["com.keep"]}), json!([always("a", &[]), session("s", far_end), minutes("m")])),
        block("sites", json!({"sites": ["example.com"]}), json!([always("a", &[])])),
        block("both", json!({"apps": ["com.mix"], "sites": ["mix.org"]}), json!([always("a", &[])])),
    ]);
    let guard: HashSet<String> = ["com.guard".to_string(), "example.com".to_string()].into();
    let nobody = none_blocked();

    // Named apps: only the named one; the all-except block also closes the others.
    assert_eq!(closers(&ask(&set, Target::App("com.named"), &nobody)), ["except/a", "except/s", "named/a"]);
    assert_eq!(closers(&ask(&set, Target::App("com.other"), &nobody)), ["except/a", "except/s"]);
    assert_eq!(closers(&ask(&set, Target::App("com.mix"), &nobody)), ["both/a", "except/a", "except/s"]);

    // All-except: the listed app is open, and so is an app that is never blocked, whatever rule closes.
    let kept = ask(&set, Target::App("com.keep"), &nobody);
    assert!(!kept.closed && kept.closing.is_empty() && kept.not_judged.is_empty(), "{kept:?}");
    let never = ask(&set, Target::App("com.guard"), &guard);
    assert_eq!(never, Decision { closed: false, closing: Vec::new(), not_judged: Vec::new() }, "a never-blocked app stays open even beside a session and a minutes rule");
    assert!(ask(&set, Target::App("com.guard"), &nobody).closed, "the same app with an empty set is closed by the all-except block");
    assert!(!ask(&set, Target::App(""), &nobody).closed, "an empty name is covered by nothing");

    // Sites: a listed name or a subdomain, never a look-alike; capitals and one trailing dot are ignored.
    for host in ["example.com", "www.example.com", "a.b.example.com", "EXAMPLE.com", "example.com.", "mix.org", "www.mix.org"] {
        assert!(ask(&set, Target::Host(host), &nobody).closed, "{host} is covered");
    }
    for host in ["notexample.com", "example.com.evil.org", "example.org", "com", "", ".", "mix.org.example"] {
        let answer = ask(&set, Target::Host(host), &nobody);
        assert!(!answer.closed, "{host:?} is not covered: {answer:?}");
    }
    assert!(ask(&set, Target::Host("example.com"), &guard).closed, "the never-blocked set holds apps; it does not open a host");

    // Apps and sites are not mixed up.
    assert!(!ask(&set, Target::Host("com.named"), &nobody).closed, "a host named like an app is not covered through `apps`");
    assert_eq!(closers(&ask(&set, Target::App("example.com"), &nobody)), ["except/a", "except/s"], "an app named like a site is not covered through `sites`, only by the all-except block");

    // A block that covers sites alone closes no app, and a named block closes no host.
    let only_sites = read_set(&[block("sites", json!({"sites": ["example.com"]}), json!([always("a", &[])]))]);
    assert!(!ask(&only_sites, Target::App("com.anything"), &nobody).closed);
    let only_apps = read_set(&[block("named", json!({"apps": ["com.named"]}), json!([outside_hours("h", &[], &[])]))]);
    assert!(!ask(&only_apps, Target::Host("com.named"), &nobody).closed);
}
