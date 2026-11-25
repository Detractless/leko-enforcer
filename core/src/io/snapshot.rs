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

fn make_read_only(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o444));
    }
}

pub struct SnapshotStore {
    root: PathBuf,
    quarantine: PathBuf,
    pinned: HashMap<String, AddonSnapshot>,
}

impl SnapshotStore {
    pub fn new(root: &Path) -> Self {
        Self::with_quarantine(root, root)
    }

    pub fn with_quarantine(root: &Path, quarantine: &Path) -> Self {
        SnapshotStore {
            root: root.to_path_buf(),
            quarantine: quarantine.to_path_buf(),
            pinned: HashMap::new(),
        }
    }

    fn safe_name(addon_id: &str) -> CoreResult<String> {
        if addon_id.is_empty()
            || addon_id.contains('/')
            || addon_id.contains('\\')
            || addon_id.contains("..")
        {
            return Err(CoreError::InvalidInput("unsafe add-on id".to_string()));
        }
        Ok(format!("{}.bin", addon_id))
    }

    pub fn pin(&mut self, spec: &CustomVerifierSpec) -> CoreResult<()> {
        let name = Self::safe_name(&spec.addon_id)?;
        let bytes = fs::read(self.root.join(&name))
            .map_err(|_| CoreError::MissingAddon(spec.addon_id.clone()))?;
        let digest = hex(&Sha256::digest(&bytes));
        if digest != spec.sha256 {
            return Err(CoreError::HashMismatch(spec.addon_id.clone()));
        }
        fs::create_dir_all(&self.quarantine)
            .map_err(|error| CoreError::Io(error.to_string()))?;
        let target = self.quarantine.join(&name);
        if !target.exists() {
            let temp = self.quarantine.join(format!(".{}.tmp", name));
            fs::write(&temp, &bytes).map_err(|error| CoreError::Io(error.to_string()))?;
            if self.quarantine != self.root {
                make_read_only(&temp);
            }
            fs::rename(&temp, &target)
                .map_err(|error| CoreError::Io(error.to_string()))?;
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
        let Ok(name) = Self::safe_name(&spec.addon_id) else {
            return false;
        };
        let Ok(bytes) = fs::read(self.quarantine.join(name)) else {
            return false;
        };
        hex(&Sha256::digest(&bytes)) == snapshot.sha256
    }
}
