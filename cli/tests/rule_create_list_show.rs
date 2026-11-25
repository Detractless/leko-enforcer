use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use leko_core::store::{load, NativeStore};
use leko_core::TechnologyManagementRule;
use leko_core::models::{Pass, Penalty, Schedule, Seal, Target};

fn call(args: &[&str]) -> (i32, String, String) {
    let raw: Vec<OsString> = args.iter().map(OsString::from).collect();
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = leko_cli::run(&raw, &mut out, &mut err);
    (code, String::from_utf8(out).unwrap(), String::from_utf8(err).unwrap())
}

fn temp_dir(label: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("leko-cli-rules-{}-{}", std::process::id(), label));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).unwrap();
    path
}

fn sample_rule(id: &str) -> TechnologyManagementRule {
    TechnologyManagementRule {
        id: id.to_string(),
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

fn write_store(path: &Path, rules: Vec<TechnologyManagementRule>) {
    leko_core::store::save(path, &NativeStore { version: 1, rules }).unwrap();
}

#[test]
fn create_rule_writes_a_complete_canonical_rule_to_the_named_store() {
    let dir = temp_dir("create");
    let store = dir.join("store.json");
    let path = store.to_str().unwrap();
    let (code, out, err) = call(&[
        "create-rule", "--store", path, "--id", "r1", "--name", "Study",
        "--target", r#"{"kind":"specific_apps","apps":["notes"]}"#,
        "--schedule", r#"{"time_based":{"kind":"always_on_schedule"},"usage_based":null}"#,
        "--penalty", r#""force_close_penalty""#,
        "--pass", r#"{"kind":"no_pass"}"#,
        "--seal", r#"{"kind":"no_seal"}"#,
        "--at", "2026-01-01T00:00:00Z",
    ]);
    assert_eq!(code, 0, "{} {}", out, err);

    let saved = load(&store).unwrap();
    assert_eq!(saved.version, 1);
    assert_eq!(saved.rules.len(), 1);
    let rule = &saved.rules[0];
    assert_eq!(rule.id, "r1");
    assert_eq!(rule.name, "Study");
    assert!(rule.enabled);
    assert_eq!(rule.created_at, "2026-01-01T00:00:00+00:00");
    assert_eq!(rule.updated_at, rule.created_at);
    assert!(matches!(rule.target, Target::SpecificApps { .. }));
    assert!(rule.penalty.is_some());
    assert_eq!(rule.pass, Pass::NoPass);
    assert_eq!(rule.seal, Seal::NoSeal);

    let whole = dir.join("whole.json");
    let (code, _, err) = call(&[
        "create-rule", "--store", whole.to_str().unwrap(), "--id", "w1", "--name", "Device",
        "--target", r#"{"kind":"whole_device","action":"lock"}"#,
        "--penalty", r#""force_close_penalty""#,
    ]);
    assert_eq!(code, 1);
    assert!(err.contains("penalty"));

    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn list_and_show_print_exact_title_case_vocabulary() {
    let dir = temp_dir("titles");
    let store = dir.join("store.json");
    write_store(&store, vec![sample_rule("r1")]);
    let path = store.to_str().unwrap();

    let (code, out, err) = call(&["list-rules", "--store", path]);
    assert_eq!(code, 0, "{}", err);
    assert!(out.contains("Target: Specific Apps"));
    assert!(out.contains("Seal: No Seal"));

    let (code, out, err) = call(&["show-rule", "--store", path, "--id", "r1"]);
    assert_eq!(code, 0, "{}", err);
    for title in [
        "Rule: Study", "Id: r1", "Enabled: true", "Target: Specific Apps",
        "Schedule: Always On Schedule", "Usage: No Usage Limit",
        "Penalty: Force Close Penalty", "Pass: No Pass", "Seal: No Seal",
        "Created At:", "Updated At:",
    ] {
        assert!(out.contains(title), "missing {}", title);
    }
    for legacy in ["Door", "Lock", "Block", "open hours"] {
        assert!(!out.contains(legacy), "legacy word {} leaked", legacy);
    }

    fs::remove_dir_all(dir).unwrap();
}
