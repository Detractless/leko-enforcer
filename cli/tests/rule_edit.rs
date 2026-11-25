use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use leko_core::models::{Pass, Penalty, Schedule, Seal, Target};
use leko_core::store::{load, NativeStore};
use leko_core::TechnologyManagementRule;

fn call(args: &[&str]) -> (i32, String, String) {
    let raw: Vec<OsString> = args.iter().map(OsString::from).collect();
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = leko_cli::run(&raw, &mut out, &mut err);
    (code, String::from_utf8(out).unwrap(), String::from_utf8(err).unwrap())
}

fn temp_dir(label: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("leko-cli-edit-{}-{}", std::process::id(), label));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).unwrap();
    path
}

fn sample(seal: Seal) -> TechnologyManagementRule {
    TechnologyManagementRule {
        id: "r1".to_string(),
        name: "Study".to_string(),
        enabled: true,
        target: Target::SpecificApps { apps: vec!["notes".to_string()] },
        schedule: Schedule::default(),
        penalty: Some(Penalty::ForceClosePenalty),
        pass: Pass::default(),
        seal,
        created_at: "2026-01-01T00:00:00Z".to_string(),
        updated_at: "2026-01-01T00:00:00Z".to_string(),
    }
}

fn store_with(path: &Path, seal: Seal) {
    leko_core::store::save(path, &NativeStore { version: 1, rules: vec![sample(seal)] }).unwrap();
}

#[test]
fn a_tightening_edit_lands_immediately() {
    let dir = temp_dir("tighten");
    let path = dir.join("store.json");
    store_with(&path, Seal::CountdownSeal {
        wait_seconds: 3600,
        unseal_requested_at: None,
    });
    let (code, out, err) = call(&[
        "edit-rule", "--store", path.to_str().unwrap(), "--id", "r1",
        "--direction", "tighten", "--field", "name", "--value", "\"Deep Study\"",
        "--at", "2026-01-01T00:00:30Z",
    ]);
    assert_eq!(code, 0, "{} {}", out, err);
    let saved = load(&path).unwrap();
    assert_eq!(saved.rules[0].name, "Deep Study");
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn weakening_pausing_and_deleting_wait_for_the_seal() {
    let dir = temp_dir("weaken");
    let closed = dir.join("closed.json");
    store_with(&closed, Seal::CountdownSeal {
        wait_seconds: 3600,
        unseal_requested_at: None,
    });
    let flag = closed.to_str().unwrap();
    for direction in ["weaken", "pause", "delete"] {
        let (code, out, err) = call(&[
            "edit-rule", "--store", flag, "--id", "r1", "--direction", direction,
            "--field", "name", "--value", "\"Changed\"", "--at", "2026-01-01T00:00:30Z",
        ]);
        assert_eq!(code, 1, "{} was allowed: {} {}", direction, out, err);
        assert!(err.contains("seal"));
    }
    assert_eq!(load(&closed).unwrap().rules[0].name, "Study");

    let open = dir.join("open.json");
    store_with(&open, Seal::CountdownSeal {
        wait_seconds: 60,
        unseal_requested_at: Some("2026-01-01T00:00:00Z".to_string()),
    });
    let (code, out, err) = call(&[
        "edit-rule", "--store", open.to_str().unwrap(), "--id", "r1",
        "--direction", "weaken", "--field", "name", "--value", "\"Changed\"",
        "--at", "2026-01-01T00:01:00Z",
    ]);
    assert_eq!(code, 0, "{} {}", out, err);
    assert_eq!(load(&open).unwrap().rules[0].name, "Changed");

    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_queued_edit_can_be_cancelled_before_it_applies() {
    let dir = temp_dir("queue");
    let path = dir.join("store.json");
    store_with(&path, Seal::QueuedDelaySeal {
        delay_seconds: 60,
        pending_edits: vec![],
    });
    let flag = path.to_str().unwrap();

    let (code, out, err) = call(&[
        "queue-edit", "--store", flag, "--id", "r1", "--field", "name",
        "--value", "\"Later\"", "--at", "2026-01-01T00:00:00Z",
    ]);
    assert_eq!(code, 0, "{} {}", out, err);
    let saved = load(&path).unwrap();
    let Seal::QueuedDelaySeal { pending_edits, .. } = &saved.rules[0].seal else {
        panic!("the queued seal was replaced");
    };
    assert_eq!(pending_edits.len(), 1);
    assert!(!pending_edits[0].cancelled);

    let (code, out, err) = call(&["cancel-edit", "--store", flag, "--id", "r1"]);
    assert_eq!(code, 0, "{} {}", out, err);
    let saved = load(&path).unwrap();
    let Seal::QueuedDelaySeal { pending_edits, .. } = &saved.rules[0].seal else {
        panic!("the queued seal was replaced");
    };
    assert!(pending_edits.iter().all(|edit| edit.cancelled));
    assert_eq!(saved.rules[0].name, "Study");

    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn delete_rule_implies_delete_direction() {
    let dir = std::env::temp_dir().join(format!("leko-m6-delete-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let store = dir.join("store.json");
    let rule = sample(Seal::NoSeal);
    leko_core::store::save(&store, &NativeStore { version: 1, rules: vec![rule] }).unwrap();
    let flag = store.to_str().unwrap();
    let (code, out, err) = call(&["delete-rule", "--store", flag, "--id", "r1"]);
    assert_eq!(code, 0, "{} {}", out, err);
    assert!(load(&store).unwrap().rules.is_empty());

    leko_core::store::save(&store, &NativeStore {
        version: 1,
        rules: vec![sample(Seal::NoSeal)],
    }).unwrap();
    let (code, out, err) = call(&["delete-rule", "--store", flag, "--id", "r1",
                                  "--direction", "delete"]);
    assert_eq!(code, 0, "{} {}", out, err);
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_sealed_delete_still_requires_the_seal() {
    let dir = std::env::temp_dir().join(format!("leko-m6-sealed-delete-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let store = dir.join("store.json");
    let sealed = Seal::CountdownSeal { wait_seconds: 3600, unseal_requested_at: None };
    leko_core::store::save(&store, &NativeStore { version: 1, rules: vec![sample(sealed)] }).unwrap();
    let (code, _, err) = call(&["delete-rule", "--store", store.to_str().unwrap(), "--id", "r1"]);
    assert_eq!(code, 1);
    assert!(err.contains("seal"));
    assert_eq!(load(&store).unwrap().rules.len(), 1);
    fs::remove_dir_all(dir).unwrap();
}
