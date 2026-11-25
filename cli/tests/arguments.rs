use std::ffi::OsString;
use std::fs;
use std::path::PathBuf;

use leko_cli::arguments::parse;
use leko_cli::services::enforcement_loop::enforce_with;
use leko_cli::services::os_executor::{MockExecutor, ObservedApp};
use leko_core::models::{Penalty, Schedule, Seal, Target};
use leko_core::TechnologyManagementRule;

fn call(args: &[&str]) -> (i32, String, String) {
    let raw: Vec<OsString> = args.iter().map(OsString::from).collect();
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = leko_cli::run(&raw, &mut out, &mut err);
    (code, String::from_utf8(out).unwrap(), String::from_utf8(err).unwrap())
}

fn temp_dir(label: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("leko-cli-args-{}-{}", std::process::id(), label));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).unwrap();
    path
}

#[test]
fn only_canonical_rule_and_infrastructure_commands_are_accepted() {
    for legacy in ["add-block", "session", "switch-off", "status", "edit-block", "delay"] {
        let (code, out, err) = call(&[legacy]);
        assert_eq!(code, 2, "legacy command {} was accepted", legacy);
        assert!(out.is_empty());
        assert!(err.contains("legacy command"));
    }
    let (code, _, err) = call(&["nonsense"]);
    assert_eq!(code, 2);
    assert!(err.contains("Unknown command"));

    let (code, out, err) = call(&["settings", "--json"]);
    assert_eq!(code, 0, "{}", err);
    assert!(out.contains("shadow_mode"));

    let (code, _, err) = call(&["create-rule", "--open-hours", "09:00"]);
    assert_eq!(code, 2);
    assert!(err.contains("legacy flag"));

    let (code, _, _) = call(&["create-rule"]);
    assert_eq!(code, 2);
}

#[test]
fn wrong_or_missing_arguments_print_usage_to_stderr_and_exit_non_zero() {
    let (code, out, err) = call(&[]);
    assert_eq!(code, 2);
    assert!(out.is_empty());
    assert!(err.contains("Usage:"));

    let (code, out, err) = call(&["create-rule", "--name"]);
    assert_eq!(code, 2);
    assert!(out.is_empty());
    assert!(err.contains("needs a value"));

    let (code, out, err) = call(&["create-rule", "--store", "--name", "x"]);
    assert_eq!(code, 2);
    assert!(out.is_empty());
    assert!(err.contains("needs a value"));

    let (code, out, err) = call(&["show-rule", "--id", "missing", "--store", "does-not-exist.json"]);
    assert_eq!(code, 1);
    assert!(out.is_empty());
    assert!(!err.is_empty());
}

#[test]
fn repeated_and_empty_input_commands_are_safe_and_idempotent() {
    let dir = temp_dir("idempotent");
    let store = dir.join("store.json");
    let path = store.to_str().unwrap();
    let target = r#"{"kind":"specific_apps","apps":["notes"]}"#;
    let command = [
        "create-rule", "--store", path, "--id", "r1", "--name", "Study",
        "--target", target, "--penalty", r#""force_close_penalty""#,
        "--at", "2026-01-01T00:00:00Z",
    ];

    let (first, out, err) = call(&command);
    assert_eq!(first, 0, "{} {}", out, err);
    let (second, _, err) = call(&command);
    assert_eq!(second, 1);
    assert!(err.contains("already exists"));

    let (code, out, _) = call(&["list-rules", "--store", path]);
    assert_eq!(code, 0);
    assert!(out.contains("1"));

    let empty = dir.join("empty.json");
    let (code, out, _) = call(&["list-rules", "--store", empty.to_str().unwrap()]);
    assert_eq!(code, 0);
    assert!(out.contains("No rules"));

    fs::remove_dir_all(dir).unwrap();
}

fn enforce_rule(target: Target) -> TechnologyManagementRule {
    TechnologyManagementRule {
        id: "r1".to_string(),
        name: "Study".to_string(),
        enabled: true,
        target,
        schedule: Schedule::default(),
        penalty: Some(Penalty::ForceClosePenalty),
        pass: Default::default(),
        seal: Seal::default(),
        created_at: "2026-01-01T00:00:00Z".to_string(),
        updated_at: "2026-01-01T00:00:00Z".to_string(),
    }
}

#[test]
fn once_and_dry_run_flags_keep_tests_safe() {
    let dir = temp_dir("loop-flags");
    let store = dir.join("store.json");
    let rule = enforce_rule(Target::SpecificApps { apps: vec!["com.example.Bad".to_string()] });
    leko_core::store::save(&store, &leko_core::store::NativeStore { version: 1, rules: vec![rule] }).unwrap();
    let path = store.to_str().unwrap();
    let argv = ["enforce", path, "--once", "--dry-run", "--at", "2026-01-01T12:00:00Z"]
        .iter().map(|value| value.to_string()).collect::<Vec<String>>();
    let parsed = parse(&argv).unwrap();
    let mut mock = MockExecutor::default();
    mock.apps.push(ObservedApp { pid: 20, bundle: Some("com.example.Bad".to_string()) });
    let mut out = Vec::new();
    let mut err = Vec::new();
    assert!(enforce_with(&parsed, &mut out, &mut err, &mut mock).is_ok());
    assert!(mock.terminated.is_empty(), "a dry run called the OS executor");
    assert!(String::from_utf8(err).unwrap().contains("Dry run"));
}

#[test]
fn usage_and_streams_follow_the_cli_contract() {
    let (code, out, err) = call(&["enforce"]);
    assert_eq!(code, 2);
    assert!(out.is_empty());
    assert!(err.contains("Usage:"));

    let dir = temp_dir("loop-usage");
    let store = dir.join("store.json");
    leko_core::store::save(&store, &leko_core::store::NativeStore { version: 1, rules: vec![] }).unwrap();
    let (code, out, err) = call(&["enforce", store.to_str().unwrap(), "--once",
                                  "--interval-ms", "not-a-number"]);
    assert_eq!(code, 2);
    assert!(out.is_empty());
    assert!(err.contains("whole number"));
}

#[test]
fn repeated_and_empty_enforce_requests_are_safe() {
    let dir = temp_dir("loop-repeat");
    let store = dir.join("store.json");
    leko_core::store::save(&store, &leko_core::store::NativeStore { version: 1, rules: vec![] }).unwrap();
    let path = store.to_str().unwrap();
    let run_once = || {
        let argv = ["enforce", path, "--once", "--at", "2026-01-01T12:00:00Z"]
            .iter().map(|value| value.to_string()).collect::<Vec<String>>();
        let parsed = parse(&argv).unwrap();
        let mut mock = MockExecutor::default();
        let mut out = Vec::new();
        let mut err = Vec::new();
        let result = enforce_with(&parsed, &mut out, &mut err, &mut mock);
        (result.is_ok(), String::from_utf8(out).unwrap(), String::from_utf8(err).unwrap())
    };
    let first = run_once();
    let second = run_once();
    assert!(first.0);
    assert!(second.0);
    assert_eq!(first.1, second.1);
    assert!(first.1.contains("Enforcement ticks: 1"));
}

#[test]
fn pass_and_delete_usage_and_streams() {
    let (code, out, err) = call(&["request-pass"]);
    assert_eq!(code, 2);
    assert!(out.is_empty());
    assert!(err.contains("--id"));

    let (code, out, err) = call(&["delete-rule"]);
    assert_eq!(code, 2);
    assert!(out.is_empty());
    assert!(err.contains("--id"));

    let raw: Vec<OsString> = ["list-addons"].iter().map(OsString::from).collect();
    let mut out = Vec::new();
    let mut err = Vec::new();
    assert_eq!(leko_cli::run(&raw, &mut out, &mut err), 0);
    assert!(String::from_utf8(err).unwrap().is_empty());
}
