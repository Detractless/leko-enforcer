use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::error::{CoreError, CoreResult};
use crate::models::CustomVerifierSpec;

#[derive(Clone, Debug)]
pub struct AddonSnapshot {
    pub addon_id: String,
    pub sha256: String,
    pub bytes: Vec<u8>,
}

fn hex(bytes: &[u8]) -> String {
    let mut text = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        text.push_str(&format!("{:02x}", byte));
    }
    text
}

pub struct SnapshotStore {
    root: PathBuf,
    pinned: HashMap<String, AddonSnapshot>,
}

impl SnapshotStore {
    pub fn new(root: &Path) -> Self {
        SnapshotStore {
            root: root.to_path_buf(),
            pinned: HashMap::new(),
        }
    }

    fn addon_path(&self, addon_id: &str) -> CoreResult<PathBuf> {
        if addon_id.is_empty()
            || addon_id.contains('/')
            || addon_id.contains('\\')
            || addon_id.contains("..")
        {
            return Err(CoreError::InvalidInput("unsafe add-on id".to_string()));
        }
        Ok(self.root.join(format!("{}.bin", addon_id)))
    }

    pub fn pin(&mut self, spec: &CustomVerifierSpec) -> CoreResult<()> {
        let path = self.addon_path(&spec.addon_id)?;
        let bytes = fs::read(&path)
            .map_err(|_| CoreError::MissingAddon(spec.addon_id.clone()))?;
        let digest = hex(&Sha256::digest(&bytes));
        if digest != spec.sha256 {
            return Err(CoreError::HashMismatch(spec.addon_id.clone()));
        }
        self.pinned.insert(
            spec.addon_id.clone(),
            AddonSnapshot {
                addon_id: spec.addon_id.clone(),
                sha256: spec.sha256.clone(),
                bytes,
            },
        );
        Ok(())
    }

    pub fn get(&self, addon_id: &str) -> Option<&AddonSnapshot> {
        self.pinned.get(addon_id)
    }

    pub fn verify(&self, spec: &CustomVerifierSpec) -> bool {
        let Some(snapshot) = self.pinned.get(&spec.addon_id) else {
            return false;
        };
        if snapshot.sha256 != spec.sha256 {
            return false;
        }
        let Ok(path) = self.addon_path(&spec.addon_id) else {
            return false;
        };
        let Ok(bytes) = fs::read(path) else {
            return false;
        };
        hex(&Sha256::digest(&bytes)) == snapshot.sha256
    }
}
