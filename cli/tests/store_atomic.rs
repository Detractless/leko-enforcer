use std::ffi::OsString;
use std::fs;

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
