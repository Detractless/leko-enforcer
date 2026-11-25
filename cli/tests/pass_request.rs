use std::ffi::OsString;
use std::fs;
use std::path::PathBuf;

use leko_core::models::{
    Pass, PassCap, PassLength, PassRequirement, Penalty, Schedule, Seal, Target,
};
use leko_cli::arguments::parse;
use leko_cli::commands::request_pass::run_with;
use leko_core::services::{load_pass_state, RecordingSleeper, Sleeper};
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

fn earned_rule(requirement: PassRequirement, cap: PassCap) -> TechnologyManagementRule {
    TechnologyManagementRule {
        id: "r1".to_string(),
        name: "Study".to_string(),
        enabled: true,
        target: Target::SpecificApps { apps: vec!["a".to_string()] },
        schedule: Schedule::default(),
        penalty: Some(Penalty::ForceClosePenalty),
        pass: Pass::EarnPass {
            requirement,
            length: PassLength::TimedPass { minutes: 1 },
            cap,
        },
        seal: Seal::default(),
        created_at: "2026-01-01T00:00:00Z".to_string(),
        updated_at: "2026-01-01T00:00:00Z".to_string(),
    }
}

fn request_with(label: &str, rule: &TechnologyManagementRule, sleeper: &mut RecordingSleeper)
    -> (PathBuf, i32, String, String) {
    let dir = std::env::temp_dir().join(format!("leko-m6-pass-{}-{}", std::process::id(), label));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let store = dir.join("store.json");
    leko_core::store::save(&store, &NativeStore { version: 1, rules: vec![rule.clone()] }).unwrap();
    let state_path = dir.join("pass-state.json");
    let argv = [
        "request-pass", "--store", store.to_str().unwrap(), "--id", "r1",
        "--pass-state", state_path.to_str().unwrap(), "--json",
        "--at", "2026-01-01T00:00:00Z",
    ].iter().map(|value| value.to_string()).collect::<Vec<String>>();
    let parsed = parse(&argv).unwrap();
    let mut out = Vec::new();
    let result = run_with(&parsed, &mut out, sleeper);
    (state_path, result.map(|_| 0).unwrap_or_else(|error| error.1),
     String::from_utf8(out).unwrap(), String::new())
}

#[test]
fn requesting_a_pass_persists_without_editing_the_rule() {
    let rule = earned_rule(PassRequirement::PhysicalPass, PassCap::UnlimitedPasses);
    let before = serde_json::to_string(&rule).unwrap();
    let mut sleeper = RecordingSleeper::default();
    let (path, code, out, _) = request_with("persist", &rule, &mut sleeper);
    assert_eq!(code, 0, "{}", out);
    let value: serde_json::Value = serde_json::from_str(out.trim()).unwrap();
    assert_eq!(value["active"], true);
    let saved = load_pass_state(&path).unwrap();
    assert!(saved.passes.contains_key("r1"));
    assert_eq!(before, serde_json::to_string(&rule).unwrap());
}

#[test]
fn earning_a_pass_never_changes_the_rule_or_its_timestamp() {
    let rule = earned_rule(PassRequirement::MentalPass, PassCap::UnlimitedPasses);
    let before = serde_json::to_string(&rule).unwrap();
    let mut sleeper = RecordingSleeper::default();
    let (_, code, _, _) = request_with("timestamp", &rule, &mut sleeper);
    assert_eq!(code, 0);
    assert_eq!(before, serde_json::to_string(&rule).unwrap());
    assert_eq!(rule.updated_at, "2026-01-01T00:00:00Z");
}

#[test]
fn a_live_countdown_waits_before_granting() {
    let rule = earned_rule(PassRequirement::CountdownPass { wait_seconds: 5 }, PassCap::UnlimitedPasses);
    let mut sleeper = RecordingSleeper::default();
    let (path, code, out, _) = request_with("countdown", &rule, &mut sleeper);
    assert_eq!(code, 0, "{}", out);
    assert_eq!(sleeper.waited_ms(), 5000);
    let value: serde_json::Value = serde_json::from_str(out.trim()).unwrap();
    assert_eq!(value["waited_ms"], 5000);
    assert!(load_pass_state(&path).unwrap().passes.contains_key("r1"));
}

#[test]
fn repeated_and_empty_pass_requests_are_safe() {
    let rule = earned_rule(PassRequirement::PhysicalPass, PassCap::LimitedPasses { max_per_window: 2 });
    let mut first = RecordingSleeper::default();
    let (_, code, _, _) = request_with("repeat-a", &rule, &mut first);
    assert_eq!(code, 0);
    let mut second = RecordingSleeper::default();
    let (_, code, _, _) = request_with("repeat-b", &rule, &mut second);
    assert_eq!(code, 0);

    let no_pass = TechnologyManagementRule { pass: Pass::NoPass, ..rule.clone() };
    let mut sleeper = RecordingSleeper::default();
    let (_, code, _, _) = request_with("no-pass", &no_pass, &mut sleeper);
    assert_eq!(code, 1);
}
