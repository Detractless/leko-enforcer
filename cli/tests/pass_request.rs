use std::ffi::OsString;
use std::fs;
use std::path::PathBuf;

use leko_core::models::{
    Pass, PassCap, PassLength, PassRequirement, Penalty, Schedule, Seal, Target,
};
use leko_core::store::{load, NativeStore};
use leko_core::TechnologyManagementRule;

fn call(args: &[&str]) -> (i32, String, String) {
    let raw: Vec<OsString> = args.iter().map(OsString::from).collect();
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = leko_cli::run(&raw, &mut out, &mut err);
    (code, String::from_utf8(out).unwrap(), String::from_utf8(err).unwrap())
}

fn temp_dir() -> PathBuf {
    let path = std::env::temp_dir().join(format!("leko-cli-pass-{}", std::process::id()));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).unwrap();
    path
}

#[test]
fn requesting_a_pass_never_mutates_the_stored_rule() {
    let dir = temp_dir();
    let store = dir.join("store.json");
    let rule = TechnologyManagementRule {
        id: "r1".to_string(),
        name: "Study".to_string(),
        enabled: true,
        target: Target::SpecificApps { apps: vec!["notes".to_string()] },
        schedule: Schedule::default(),
        penalty: Some(Penalty::ForceClosePenalty),
        pass: Pass::EarnPass {
            requirement: PassRequirement::PhysicalPass,
            length: PassLength::TimedPass { minutes: 30 },
            cap: PassCap::LimitedPasses { max_per_window: 2 },
        },
        seal: Seal::default(),
        created_at: "2026-01-01T00:00:00Z".to_string(),
        updated_at: "2026-01-01T00:00:00Z".to_string(),
    };
    leko_core::store::save(&store, &NativeStore { version: 1, rules: vec![rule] }).unwrap();
    let before = fs::read(&store).unwrap();

    let (code, out, err) = call(&[
        "request-pass", "--store", store.to_str().unwrap(), "--id", "r1",
        "--at", "2026-01-01T00:05:00Z",
    ]);
    assert_eq!(code, 0, "{} {}", out, err);
    assert!(out.contains("Earn Pass"));
    assert!(out.contains("Active: true"));

    assert_eq!(fs::read(&store).unwrap(), before, "the stored rule changed");
    let saved = load(&store).unwrap();
    assert!(matches!(saved.rules[0].pass, Pass::EarnPass { .. }));

    let (code, out, err) = call(&[
        "request-pass", "--store", store.to_str().unwrap(), "--id", "r1",
        "--json", "--at", "2026-01-01T00:05:00Z",
    ]);
    assert_eq!(code, 0, "{} {}", out, err);
    let value: serde_json::Value = serde_json::from_str(out.trim()).unwrap();
    assert_eq!(value["active"], true);

    fs::remove_dir_all(dir).unwrap();
}
