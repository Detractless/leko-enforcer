use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::{CoreError, CoreResult};
use crate::models::CustomVerifierSpec;

pub const SNAPSHOT_VERSION: u32 = 1;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BundleFile {
    pub name: String,
    pub sha256: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BundleManifest {
    pub version: u32,
    pub addon_id: String,
    pub config_sha256: String,
    pub files: Vec<BundleFile>,
}

pub fn hex(bytes: &[u8]) -> String {
    let mut text = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        text.push_str(&format!("{:02x}", byte));
    }
    text
}

pub fn digest_bytes(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

pub fn manifest_digest(manifest: &BundleManifest) -> String {
    let mut combined = format!("{}|{}|{}", manifest.version, manifest.addon_id, manifest.config_sha256);
    for file in &manifest.files {
        combined.push_str(&format!("|{}:{}", file.name, file.sha256));
    }
    digest_bytes(combined.as_bytes())
}

fn read_manifest(source: &Path) -> CoreResult<BundleManifest> {
    let path = source.join("manifest.json");
    let bytes = fs::read(&path)
        .map_err(|_| CoreError::MissingAddon(path.display().to_string()))?;
    serde_json::from_slice(&bytes)
        .map_err(|error| CoreError::Parse(format!("{}: {}", path.display(), error)))
}

fn validate(source: &Path, manifest: &BundleManifest, spec: &CustomVerifierSpec) -> CoreResult<String> {
    if manifest.version != SNAPSHOT_VERSION {
        return Err(CoreError::UnknownVersion(manifest.version));
    }
    if manifest.addon_id != spec.addon_id {
        return Err(CoreError::InvalidInput("the manifest names a different add-on".to_string()));
    }
    let config_hash = digest_bytes(serde_json::to_vec(&spec.config).unwrap_or_default().as_slice());
    if manifest.config_sha256 != config_hash {
        return Err(CoreError::HashMismatch(spec.addon_id.clone()));
    }
    for file in &manifest.files {
        if file.name.contains('/') || file.name.contains('\\') || file.name.contains("..") {
            return Err(CoreError::InvalidInput("unsafe file name in manifest".to_string()));
        }
        let bytes = fs::read(source.join(&file.name))
            .map_err(|_| CoreError::MissingAddon(file.name.clone()))?;
        if digest_bytes(&bytes) != file.sha256 {
            return Err(CoreError::HashMismatch(file.name.clone()));
        }
    }
    Ok(manifest_digest(manifest))
}

fn snapshot_dir(quarantine: &Path, spec: &CustomVerifierSpec, digest: &str) -> PathBuf {
    quarantine.join(format!("{}-{}", spec.addon_id, digest))
}

fn make_read_only(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o555));
    }
}

pub fn pin(source: &Path, quarantine: &Path, spec: &CustomVerifierSpec) -> CoreResult<PathBuf> {
    let manifest = read_manifest(source)?;
    let digest = validate(source, &manifest, spec)?;
    if digest != spec.sha256 {
        return Err(CoreError::HashMismatch(spec.addon_id.clone()));
    }
    let target = snapshot_dir(quarantine, spec, &digest);
    if target.exists() {
        return Ok(target);
    }
    fs::create_dir_all(quarantine).map_err(|error| CoreError::Io(error.to_string()))?;
    let temp = quarantine.join(format!(".{}.{}.tmp", spec.addon_id, digest));
    if temp.exists() {
        fs::remove_dir_all(&temp).map_err(|error| CoreError::Io(error.to_string()))?;
    }
    fs::create_dir(&temp).map_err(|error| CoreError::Io(error.to_string()))?;
    for file in &manifest.files {
        let bytes = fs::read(source.join(&file.name))
            .map_err(|error| CoreError::Io(error.to_string()))?;
        fs::write(temp.join(&file.name), bytes)
            .map_err(|error| CoreError::Io(error.to_string()))?;
    }
    fs::write(temp.join("manifest.json"), serde_json::to_vec(&manifest).unwrap_or_default())
        .map_err(|error| CoreError::Io(error.to_string()))?;
    fs::rename(&temp, &target).map_err(|error| CoreError::Io(error.to_string()))?;
    make_read_only(&target);
    Ok(target)
}

pub fn verify(quarantine: &Path, spec: &CustomVerifierSpec) -> bool {
    let Ok(entries) = fs::read_dir(quarantine) else {
        return false;
    };
    let prefix = format!("{}-", spec.addon_id);
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !name.starts_with(&prefix) {
            continue;
        }
        let path = entry.path();
        if !path.is_dir() {
            return false;
        }
        let Ok(bytes) = fs::read(path.join("manifest.json")) else {
            return false;
        };
        let Ok(manifest) = serde_json::from_slice::<BundleManifest>(&bytes) else {
            return false;
        };
        if manifest_digest(&manifest) != spec.sha256 {
            return false;
        }
        for file in &manifest.files {
            let Ok(stored) = fs::read(path.join(&file.name)) else {
                return false;
            };
            if digest_bytes(&stored) != file.sha256 {
                return false;
            }
        }
        return true;
    }
    false
}
