use leko_core::models::{Penalty, Schedule, Seal, Target};
use leko_core::store::{commit_temp, load, save, write_temp, NativeStore};
use leko_core::services::{load_pass_state, save_pass_state, PassStateFile, StoredPass};
use leko_core::TechnologyManagementRule;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

fn sample() -> TechnologyManagementRule {
    TechnologyManagementRule {
        id: "rule-1".into(),
        name: "Study".into(),
        enabled: true,
        target: Target::SpecificApps { apps: vec!["a".into()] },
        schedule: Schedule::default(),
        penalty: Some(Penalty::ForceClosePenalty),
        pass: Default::default(),
        seal: Seal::NoSeal,
        created_at: "2026-01-01T00:00:00Z".into(),
        updated_at: "2026-01-01T00:00:00Z".into(),
    }
}

fn directory() -> PathBuf {
    let path = std::env::temp_dir().join(format!("leko-store-{}", std::process::id()));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).unwrap();
    path
}

#[test]
fn a_crash_half_way_through_a_save_leaves_the_old_store_readable() {
    let dir = directory();
    let path = dir.join("store.json");

    let first = NativeStore::empty();
    save(&path, &first).unwrap();
    let original = fs::read(&path).unwrap();

    let mut second = first.clone();
    second.rules.push(sample());
    let temp = write_temp(&path, &second).unwrap();
    assert!(temp.exists(), "the temp file should hold the pending save");
    assert_eq!(fs::read(&path).unwrap(), original, "the old store was changed before commit");
    assert_eq!(load(&path).unwrap(), first);

    commit_temp(&temp, &path).unwrap();
    assert_eq!(load(&path).unwrap(), second);

    let missing = dir.join("missing.json");
    assert_eq!(load(&missing).unwrap(), NativeStore::empty());

    let empty = dir.join("empty.json");
    fs::write(&empty, b"").unwrap();
    assert!(load(&empty).is_err(), "an empty store must be reported");

    let corrupt = dir.join("corrupt.json");
    fs::write(&corrupt, b"{not json").unwrap();
    let before = fs::read(&corrupt).unwrap();
    assert!(load(&corrupt).is_err());
    assert_eq!(fs::read(&corrupt).unwrap(), before, "a corrupt store must never be overwritten");

    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_partial_pass_state_write_keeps_the_old_state() {
    let dir = std::env::temp_dir().join(format!("leko-pass-state-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("rules.pass-state.json");
    let original = PassStateFile {
        version: 1,
        passes: HashMap::from([("r1".to_string(), StoredPass {
            rule_id: "r1".to_string(),
            requirement: "PhysicalPass".to_string(),
            earned_at: "2026-01-01T00:00:00Z".to_string(),
            window_key: "2026-01-01".to_string(),
            used_in_window: 0,
        })]),
    };
    save_pass_state(&path, &original).unwrap();
    let before = fs::read(&path).unwrap();

    let changed = PassStateFile { passes: HashMap::new(), ..original.clone() };
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o555)).unwrap();
    }
    assert!(save_pass_state(&path, &changed).is_err());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o755)).unwrap();
    }
    assert_eq!(fs::read(&path).unwrap(), before);
    assert_eq!(load_pass_state(&path).unwrap(), original);
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_partial_usage_state_write_keeps_the_old_state() {
    use leko_core::services::{
        load_usage_state, save_usage_state, usage_state_path, RuleUsage, UsageStateFile,
    };
    let dir = std::env::temp_dir().join(format!("leko-usage-state-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let store_path = dir.join("rules.json");
    let path = usage_state_path(&store_path);
    assert_eq!(path, dir.join("rules.usage-state.json"));

    let original = UsageStateFile {
        version: 1,
        rules: HashMap::from([("r1".to_string(), RuleUsage {
            rule_id: "r1".to_string(),
            period_key: "2026-01-01".to_string(),
            active_seconds: 120,
            sessions: 1,
            session_open: true,
            last_active_at: Some("2026-01-01T12:00:00Z".to_string()),
            away_since: None,
            engagement_seconds: 30,
            engagement_window_key: "2026-01-01".to_string(),
            last_engagement_at: None,
        })]),
    };
    save_usage_state(&path, &original).unwrap();
    let before = fs::read(&path).unwrap();

    let changed = UsageStateFile { rules: HashMap::new(), ..original.clone() };
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o555)).unwrap();
    }
    assert!(save_usage_state(&path, &changed).is_err());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o755)).unwrap();
    }
    assert_eq!(fs::read(&path).unwrap(), before);
    assert_eq!(load_usage_state(&path).unwrap(), original);
    fs::remove_dir_all(dir).unwrap();
}
