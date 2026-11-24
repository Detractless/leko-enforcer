use leko_core::models::{Penalty, Schedule, Seal, Target};
use leko_core::store::{commit_temp, load, save, write_temp, NativeStore};
use leko_core::TechnologyManagementRule;
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
