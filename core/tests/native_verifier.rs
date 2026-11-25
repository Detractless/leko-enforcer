use sha2::{Digest, Sha256};
use leko_core::io::snapshot::{AddonSnapshot, SnapshotStore};
use leko_core::models::CustomVerifierSpec;
use leko_core::services::{
    pin_bundle, verify_pin, AddonRunner, BundleManifest, Challenge, NonceGuard, NonceIssuer,
    Verification, VerifierHook,
};
use std::fs;
use std::path::PathBuf;

struct Accept;
impl AddonRunner for Accept {
    fn run(&self, _snapshot: &AddonSnapshot, _config: &serde_json::Value, _challenge: &Challenge)
        -> Result<bool, String> {
        Ok(true)
    }
}

struct Crash;
impl AddonRunner for Crash {
    fn run(&self, _snapshot: &AddonSnapshot, _config: &serde_json::Value, _challenge: &Challenge)
        -> Result<bool, String> {
        Err("runner crashed".to_string())
    }
}

fn hash(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut text = String::with_capacity(64);
    for byte in digest {
        text.push_str(&format!("{:02x}", byte));
    }
    text
}

fn setup(label: &str) -> (PathBuf, CustomVerifierSpec, SnapshotStore) {
    let directory = std::env::temp_dir().join(format!("leko-verifier-{}-{}", std::process::id(), label));
    let _ = fs::remove_dir_all(&directory);
    fs::create_dir_all(&directory).unwrap();
    fs::write(directory.join("checker.bin"), b"trusted code").unwrap();
    let spec = CustomVerifierSpec {
        addon_id: "checker".into(),
        sha256: hash(b"trusted code"),
        config: serde_json::json!({"mode": "strict"}),
    };
    let mut snapshots = SnapshotStore::new(&directory);
    snapshots.pin(&spec).unwrap();
    (directory, spec, snapshots)
}

#[test]
fn every_verifier_failure_returns_not_satisfied() {
    let (directory, spec, snapshots) = setup("failures");
    let mut guard = NonceGuard::default();
    let mut issuer = NonceIssuer::default();
    let mut hook = VerifierHook::new(&snapshots, &mut guard);

    assert_eq!(hook.evaluate(&spec, None, &issuer.issue()), Verification::NotSatisfied);
    assert_eq!(hook.evaluate(&spec, Some(&Crash), &issuer.issue()), Verification::NotSatisfied);

    let wrong_hash = CustomVerifierSpec { sha256: "00".repeat(32), ..spec.clone() };
    assert_eq!(hook.evaluate(&wrong_hash, Some(&Accept), &issuer.issue()), Verification::NotSatisfied);

    let missing = CustomVerifierSpec { addon_id: "absent".into(), ..spec.clone() };
    assert_eq!(hook.evaluate(&missing, Some(&Accept), &issuer.issue()), Verification::NotSatisfied);

    assert_eq!(hook.evaluate(&spec, Some(&Accept), &issuer.issue()), Verification::Satisfied);
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn a_replayed_nonce_or_a_changed_snapshot_is_refused() {
    let (directory, spec, snapshots) = setup("replay");
    let mut guard = NonceGuard::default();
    let mut issuer = NonceIssuer::default();
    let mut hook = VerifierHook::new(&snapshots, &mut guard);

    let nonce = issuer.issue();
    assert_eq!(hook.evaluate(&spec, Some(&Accept), &nonce), Verification::Satisfied);
    assert_eq!(hook.evaluate(&spec, Some(&Accept), &nonce), Verification::NotSatisfied);

    fs::write(directory.join("checker.bin"), b"tampered code").unwrap();
    assert_eq!(hook.evaluate(&spec, Some(&Accept), &issuer.issue()), Verification::NotSatisfied);
    fs::remove_dir_all(directory).unwrap();
}

fn clean(path: &std::path::Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(entries) = fs::read_dir(path) {
            for entry in entries.flatten() {
                let child = entry.path();
                if child.is_dir() {
                    let _ = fs::set_permissions(&child, fs::Permissions::from_mode(0o755));
                    clean(&child);
                } else {
                    let _ = fs::set_permissions(&child, fs::Permissions::from_mode(0o644));
                }
            }
        }
    }
    let _ = fs::remove_dir_all(path);
}

fn bundle(label: &str) -> (PathBuf, PathBuf, CustomVerifierSpec) {
    let base = std::env::temp_dir().join(format!("leko-pin-{}-{}", std::process::id(), label));
    clean(&base);
    let source = base.join("source");
    let quarantine = base.join("quarantine");
    fs::create_dir_all(&source).unwrap();
    fs::create_dir_all(&quarantine).unwrap();

    let payload = b"payload";
    let config = serde_json::json!({"mode": "strict"});
    let manifest = serde_json::json!({
        "version": 1,
        "addon_id": "checker",
        "config_sha256": hash(&serde_json::to_vec(&config).unwrap()),
        "files": [{"name": "addon.sh", "sha256": hash(payload)}],
    });
    let parsed: BundleManifest = serde_json::from_value(manifest.clone()).unwrap();
    let digest = leko_core::services::manifest_digest(&parsed);
    let spec = CustomVerifierSpec {
        addon_id: "checker".into(),
        sha256: digest,
        config,
    };
    fs::write(source.join("addon.sh"), payload).unwrap();
    fs::write(source.join("manifest.json"), serde_json::to_vec(&manifest).unwrap()).unwrap();
    pin_bundle(&source, &quarantine, &spec).unwrap();
    (source, quarantine, spec)
}

#[test]
fn pin_copies_a_hashed_bundle_into_the_snapshot_root() {
    let (source, quarantine, spec) = bundle("copy");
    assert!(source.join("addon.sh").exists());
    let pinned = fs::read_dir(&quarantine).unwrap().count();
    assert_eq!(pinned, 1);
    assert!(verify_pin(&quarantine, &spec));
    clean(source.parent().unwrap());
}

#[test]
fn source_edits_cannot_change_a_pinned_snapshot() {
    let (source, quarantine, spec) = bundle("immutable");
    fs::write(source.join("addon.sh"), b"tampered").unwrap();
    let manifest = serde_json::json!({
        "version": 1,
        "addon_id": "checker",
        "config_sha256": hash(&serde_json::to_vec(&spec.config).unwrap()),
        "files": [{"name": "addon.sh", "sha256": hash(b"tampered")}],
    });
    fs::write(source.join("manifest.json"), serde_json::to_vec(&manifest).unwrap()).unwrap();
    assert!(verify_pin(&quarantine, &spec));
    clean(source.parent().unwrap());
}

#[test]
fn a_partial_pin_write_keeps_the_previous_snapshot() {
    let (source, quarantine, spec) = bundle("partial");
    let changed = CustomVerifierSpec {
        config: serde_json::json!({"mode": "other"}),
        ..spec.clone()
    };
    let manifest = serde_json::json!({
        "version": 1,
        "addon_id": "checker",
        "config_sha256": hash(&serde_json::to_vec(&changed.config).unwrap()),
        "files": [{"name": "addon.sh", "sha256": hash(b"payload")}],
    });
    fs::write(source.join("manifest.json"), serde_json::to_vec(&manifest).unwrap()).unwrap();
    let parsed: BundleManifest = serde_json::from_value(manifest).unwrap();
    let changed = CustomVerifierSpec { sha256: leko_core::services::manifest_digest(&parsed), ..changed };

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&quarantine, fs::Permissions::from_mode(0o555)).unwrap();
    }
    assert!(pin_bundle(&source, &quarantine, &changed).is_err());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&quarantine, fs::Permissions::from_mode(0o755)).unwrap();
    }
    assert!(verify_pin(&quarantine, &spec));
    clean(source.parent().unwrap());
}

#[test]
fn a_broken_manifest_is_reported_without_overwriting() {
    let (source, quarantine, spec) = bundle("broken");
    let path = source.join("manifest.json");
    fs::write(&path, b"{not json").unwrap();
    let before = fs::read(&path).unwrap();
    assert!(pin_bundle(&source, &quarantine, &spec).is_err());
    assert_eq!(fs::read(&path).unwrap(), before);
    assert!(verify_pin(&quarantine, &spec));
    clean(source.parent().unwrap());
}

#[test]
fn an_unknown_snapshot_version_is_refused() {
    let (source, quarantine, spec) = bundle("version");
    let manifest = serde_json::json!({
        "version": 99,
        "addon_id": "checker",
        "config_sha256": hash(&serde_json::to_vec(&spec.config).unwrap()),
        "files": [{"name": "addon.sh", "sha256": hash(b"payload")}],
    });
    fs::write(source.join("manifest.json"), serde_json::to_vec(&manifest).unwrap()).unwrap();
    assert!(pin_bundle(&source, &quarantine, &spec).is_err());
    assert!(verify_pin(&quarantine, &spec));
    clean(source.parent().unwrap());
}
