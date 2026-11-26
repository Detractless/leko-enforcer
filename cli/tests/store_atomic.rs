use std::ffi::OsString;
use std::fs;

use leko_core::models::{Pass, Penalty, Schedule, Seal, Target};
use leko_core::store::{load, NativeStore};
use leko_core::TechnologyManagementRule;
use leko_cli::arguments::parse;
use leko_cli::services::enforcement_loop::enforce_with;
use leko_cli::services::os_executor::MockExecutor;

fn call(args: &[&str]) -> (i32, String, String) {
    let raw: Vec<OsString> = args.iter().map(OsString::from).collect();
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = leko_cli::run(&raw, &mut out, &mut err);
    (code, String::from_utf8(out).unwrap(), String::from_utf8(err).unwrap())
}

fn sample() -> TechnologyManagementRule {
    TechnologyManagementRule {
        id: "r1".to_string(),
        name: "Study".to_string(),
        enabled: true,
        target: Target::SpecificApps { apps: vec!["notes".to_string()] },
        schedule: Schedule::default(),
        penalty: Some(Penalty::ForceClosePenalty),
        pass: Pass::default(),
        seal: Seal::default(),
        created_at: "2026-01-01T00:00:00Z".to_string(),
        updated_at: "2026-01-01T00:00:00Z".to_string(),
    }
}

#[test]
fn a_broken_store_is_reported_and_a_partial_save_keeps_the_old_one() {
    let dir = std::env::temp_dir().join(format!("leko-cli-store-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let store = dir.join("store.json");
    leko_core::store::save(&store, &NativeStore { version: 1, rules: vec![sample()] }).unwrap();
    let good = fs::read(&store).unwrap();

    let empty = dir.join("empty.json");
    fs::write(&empty, b"").unwrap();
    let (code, out, err) = call(&["show-rule", "--store", empty.to_str().unwrap(), "--id", "r1"]);
    assert_eq!(code, 1);
    assert!(out.is_empty());
    assert!(err.contains("empty"));

    fs::write(&store, b"{not json").unwrap();
    let broken = fs::read(&store).unwrap();
    let (code, out, err) = call(&["show-rule", "--store", store.to_str().unwrap(), "--id", "r1"]);
    assert_eq!(code, 1);
    assert!(out.is_empty());
    assert!(!err.is_empty());
    assert_eq!(fs::read(&store).unwrap(), broken, "the corrupt store was overwritten");

    fs::write(&store, &good).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o555)).unwrap();
    }
    let (code, _, err) = call(&[
        "create-rule", "--store", store.to_str().unwrap(), "--id", "r2", "--name", "New",
        "--target", r#"{"kind":"specific_apps","apps":["a"]}"#,
    ]);
    assert_eq!(code, 1, "a failed save should be reported: {}", err);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o755)).unwrap();
    }
    assert_eq!(load(&store).unwrap().rules.len(), 1, "the old store was lost");
    assert_eq!(load(&store).unwrap().rules[0].id, "r1");

    fs::remove_dir_all(dir).unwrap();
}

fn loop_dir(label: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("leko-loop-store-{}-{}", std::process::id(), label));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn a_corrupt_or_missing_store_keeps_the_last_valid_state() {
    let dir = loop_dir("corrupt");
    let store = dir.join("store.json");
    let valid = sample();
    leko_core::store::save(&store, &NativeStore { version: 1, rules: vec![valid] }).unwrap();
    let argv = ["enforce", store.to_str().unwrap(), "--once", "--at", "2026-01-01T12:00:00Z"]
        .iter().map(|value| value.to_string()).collect::<Vec<String>>();
    let parsed = parse(&argv).unwrap();
    let mut mock = MockExecutor::default();
    assert!(enforce_with(&parsed, &mut Vec::new(), &mut Vec::new(), &mut mock).is_ok());

    fs::write(&store, b"{not json").unwrap();
    let before = fs::read(&store).unwrap();
    let mut out = Vec::new();
    let mut err = Vec::new();
    let failure = enforce_with(&parsed, &mut out, &mut err, &mut mock).unwrap_err();
    assert!(out.is_empty());
    assert!(!failure.0.is_empty());
    assert_eq!(fs::read(&store).unwrap(), before);
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn native_store_version_is_preserved() {
    let dir = loop_dir("version");
    let store = dir.join("store.json");
    leko_core::store::save(&store, &NativeStore { version: 1, rules: vec![sample()] }).unwrap();
    let argv = ["enforce", store.to_str().unwrap(), "--once", "--at", "2026-01-01T12:00:00Z"]
        .iter().map(|value| value.to_string()).collect::<Vec<String>>();
    let parsed = parse(&argv).unwrap();
    let mut mock = MockExecutor::default();
    assert!(enforce_with(&parsed, &mut Vec::new(), &mut Vec::new(), &mut mock).is_ok());
    let saved: serde_json::Value = serde_json::from_slice(&fs::read(&store).unwrap()).unwrap();
    assert_eq!(saved["version"], 1);
    assert!(saved.get("outsideHours").is_none());
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn corrupt_usage_state_and_empty_ticks_are_safe() {
    let dir = loop_dir("usage-corrupt");
    let store = dir.join("store.json");
    let usage = dir.join("store.usage-state.json");
    let other = dir.join("other-usage.json");
    leko_core::store::save(&store, &NativeStore { version: 1, rules: vec![sample()] }).unwrap();
    fs::write(&usage, b"{corrupt json").unwrap();
    fs::write(&other, b"{\"version\":1,\"rules\":{}}").unwrap();
    let before_store = fs::read(&store).unwrap();
    let before_other = fs::read(&other).unwrap();

    let argv = ["enforce", store.to_str().unwrap(), "--once", "--at", "2026-01-01T12:00:00Z"]
        .iter().map(|v| v.to_string()).collect::<Vec<String>>();
    let mut mock = MockExecutor::default();
    let mut err = Vec::new();
    assert!(enforce_with(&parse(&argv).unwrap(), &mut Vec::new(), &mut err, &mut mock).is_ok());
    assert!(String::from_utf8(err).unwrap().contains("Usage state unreadable"));
    assert_eq!(fs::read(&usage).unwrap(), b"{corrupt json");
    assert_eq!(fs::read(&store).unwrap(), before_store);
    assert_eq!(fs::read(&other).unwrap(), before_other);

    let empty_store = dir.join("empty.json");
    leko_core::store::save(&empty_store, &NativeStore { version: 1, rules: vec![] }).unwrap();
    let empty_argv = ["enforce", empty_store.to_str().unwrap(), "--once", "--at", "2026-01-01T12:00:00Z"]
        .iter().map(|v| v.to_string()).collect::<Vec<String>>();
    for _ in 0..2 {
        let mut out = Vec::new();
        assert!(enforce_with(&parse(&empty_argv).unwrap(), &mut out, &mut Vec::new(), &mut mock).is_ok());
        assert!(String::from_utf8(out).unwrap().contains("Enforcement ticks: 1"));
    }
    fs::remove_dir_all(dir).unwrap();
}
