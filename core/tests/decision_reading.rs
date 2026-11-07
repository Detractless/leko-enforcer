//! The readers keep what the decision needs: the switch, which apps and sites a block covers, the
//! weekdays, the allowed stretches and the end of a session (AC-010). What they accept and refuse is
//! the same as before; the corpus and extension suites check that.

mod block_builder;
mod zip_writer;

use block_builder::{always, block, block_with, minutes, moment, outside_hours, read_set, session};
use leko_core::io::zip_check::{check_zip_in_mode, ZipVerdict};
use leko_core::models::specs::{AppCover, ReadMode};
use serde_json::json;
use std::collections::HashSet;
use zip_writer::zip_of;

#[test]
fn the_readers_keep_what_the_decision_needs() {
    let set = read_set(&[
        block(
            "first",
            json!({"apps": ["com.a", "com.b"]}),
            json!([
                always("days", &["tue", "mon"]),
                outside_hours("hours", &[], &[("22:00", "06:00"), ("12:00", "13:30")]),
                session("until", "2026-11-05T18:30:00Z"),
                always("all_seven", &["sun", "sat", "fri", "thu", "wed", "tue", "mon"]),
                minutes("counted"),
            ]),
        ),
        block_with("second", json!({"allAppsExcept": ["com.x"]}), json!([always("r", &[])]), json!({"on": false})),
        block_with("third", json!({"sites": ["example.com", "b.org"]}), json!([always("r", &[])]), json!({"on": true})),
        block("fourth", json!({"apps": ["com.c"], "sites": ["c.org"]}), json!([always("r", &[])])),
    ]);

    let first = set.blocks.get("first").expect("first");
    assert!(first.on, "no `on` field is on");
    assert_eq!(first.apps, AppCover::Named(vec!["com.a".to_string(), "com.b".to_string()]));
    assert!(first.sites.is_empty());
    let days = &first.rules[0];
    assert_eq!((days.id.as_str(), days.kind.as_str()), ("days", "always"));
    assert_eq!(days.days, ["mon", "tue"], "weekdays come back in the order of the week");
    assert!(days.allowed.is_empty() && days.ends_at.is_none());
    let hours = &first.rules[1];
    assert_eq!(hours.allowed, [(1320, 360), (720, 810)], "from and to in minutes after midnight, in the order written");
    assert!(hours.days.is_empty(), "no weekdays named is every day");
    let until = &first.rules[2];
    assert_eq!(until.ends_at, Some(moment("2026-11-05T18:30:00Z")));
    assert!(until.days.is_empty() && until.allowed.is_empty());
    assert!(first.rules[3].days.is_empty(), "all seven weekdays named is every day");
    assert_eq!((first.rules[4].kind.as_str(), first.rules[4].ends_at, first.rules[4].allowed.len()), ("minutes", None, 0));

    let second = set.blocks.get("second").expect("second");
    assert!(!second.on, "`on: false` is off");
    assert_eq!(second.apps, AppCover::AllExcept(vec!["com.x".to_string()]));
    let third = set.blocks.get("third").expect("third");
    assert!(third.on);
    assert_eq!(third.apps, AppCover::Nothing, "a block of sites alone covers no app");
    assert_eq!(third.sites, ["example.com", "b.org"]);
    let fourth = set.blocks.get("fourth").expect("fourth");
    assert_eq!((fourth.apps.clone(), fourth.sites.clone()), (AppCover::Named(vec!["com.c".to_string()]), vec!["c.org".to_string()]));

    // The phone's own reading keeps the same parts for what the phone accepts.
    let file = block_with("phone", json!({"allAppsExcept": []}), json!([outside_hours("h", &["fri"], &[("08:00", "24:00")])]), json!({"on": false})).to_string();
    match check_zip_in_mode(&zip_of(&[("GUIDE.md", "guide"), ("blocks/phone.json", &file)]), &HashSet::new(), ReadMode::Phone) {
        ZipVerdict::Read(read) => {
            let phone = read.set.blocks.get("phone").expect("phone");
            assert!(!phone.on);
            assert_eq!(phone.apps, AppCover::AllExcept(Vec::new()));
            assert_eq!((phone.rules[0].days.clone(), phone.rules[0].allowed.clone()), (vec!["fri".to_string()], vec![(480, 1440)]));
        }
        ZipVerdict::Refused(refusal) => panic!("the phone's reading refused a block it accepts: {}", refusal.sentence),
    }
}
