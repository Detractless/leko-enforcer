use std::ffi::OsString;
use std::fs;

use leko_core::models::{Pass, Penalty, Schedule, Seal, Target, TimeBasedSchedule};
use leko_core::store::NativeStore;
use leko_core::TechnologyManagementRule;

fn call(args: &[&str]) -> (i32, String, String) {
    let raw: Vec<OsString> = args.iter().map(OsString::from).collect();
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = leko_cli::run(&raw, &mut out, &mut err);
    (code, String::from_utf8(out).unwrap(), String::from_utf8(err).unwrap())
}

#[test]
fn status_and_evaluation_are_deterministic_under_an_injected_timestamp() {
    let dir = std::env::temp_dir().join(format!("leko-cli-status-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let store = dir.join("store.json");
    let rule = TechnologyManagementRule {
        id: "r1".to_string(),
        name: "Study".to_string(),
        enabled: true,
        target: Target::SpecificWebsites { sites: vec!["site.test".to_string()] },
        schedule: Schedule {
            time_based: TimeBasedSchedule::OneTimeSchedule {
                start_at: "2026-01-01T00:00:00Z".to_string(),
                end_at: "2026-01-02T00:00:00Z".to_string(),
            },
            usage_based: None,
        },
        penalty: Some(Penalty::ForceClosePenalty),
        pass: Pass::default(),
        seal: Seal::default(),
        created_at: "2026-01-01T00:00:00Z".to_string(),
        updated_at: "2026-01-01T00:00:00Z".to_string(),
    };
    leko_core::store::save(&store, &NativeStore { version: 1, rules: vec![rule] }).unwrap();
    let flag = store.to_str().unwrap();

    let first = call(&["evaluate-rule", "--store", flag, "--id", "r1", "--at", "2026-01-01T12:00:00Z"]);
    let second = call(&["evaluate-rule", "--store", flag, "--id", "r1", "--at", "2026-01-01T12:00:00Z"]);
    assert_eq!(first.0, 0, "{} {}", first.1, first.2);
    assert_eq!(first.1, second.1, "the same timestamp gave different answers");
    assert!(first.1.contains("Enforced: true"));

    let later = call(&["check-status", "--store", flag, "--id", "r1", "--at", "2026-01-03T00:00:00Z"]);
    assert_eq!(later.0, 0, "{} {}", later.1, later.2);
    assert!(later.1.contains("Enforced: false"));
    assert!(later.1.contains("OutsideSchedule"));

    let json = call(&[
        "evaluate-rule", "--store", flag, "--id", "r1", "--json",
        "--at", "2026-01-01T12:00:00Z",
    ]);
    assert_eq!(json.0, 0, "{} {}", json.1, json.2);
    let value: serde_json::Value = serde_json::from_str(json.1.trim()).unwrap();
    assert_eq!(value["enforced"], true);

    fs::remove_dir_all(dir).unwrap();
}
