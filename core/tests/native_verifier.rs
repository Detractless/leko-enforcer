use sha2::{Digest, Sha256};
use leko_core::io::snapshot::{AddonSnapshot, SnapshotStore};
use leko_core::models::CustomVerifierSpec;
use leko_core::services::{AddonRunner, NonceGuard, NonceIssuer, Verification, VerifierHook};
use std::fs;
use std::path::PathBuf;

struct Accept;
impl AddonRunner for Accept {
    fn run(&self, _snapshot: &AddonSnapshot, _config: &serde_json::Value) -> Result<bool, String> {
        Ok(true)
    }
}

struct Crash;
impl AddonRunner for Crash {
    fn run(&self, _snapshot: &AddonSnapshot, _config: &serde_json::Value) -> Result<bool, String> {
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
