use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::error::{CoreError, CoreResult};
use crate::store::native_json::NativeStore;

fn temp_path(path: &Path) -> CoreResult<PathBuf> {
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| CoreError::InvalidInput("store path has no file name".to_string()))?;
    Ok(path.with_file_name(format!(".{}.tmp", name)))
}

pub fn load(path: &Path) -> CoreResult<NativeStore> {
    if !path.exists() {
        return Ok(NativeStore::empty());
    }
    let bytes = fs::read(path).map_err(|error| CoreError::Io(error.to_string()))?;
    if bytes.is_empty() {
        return Err(CoreError::Parse("the store file is empty".to_string()));
    }
    NativeStore::from_bytes(&bytes)
}

pub fn write_temp(path: &Path, store: &NativeStore) -> CoreResult<PathBuf> {
    let bytes = store.to_bytes()?;
    let temp = temp_path(path)?;
    let mut file = File::create(&temp).map_err(|error| CoreError::Io(error.to_string()))?;
    file.write_all(&bytes)
        .map_err(|error| CoreError::Io(error.to_string()))?;
    file.sync_all()
        .map_err(|error| CoreError::Io(error.to_string()))?;
    Ok(temp)
}

pub fn commit_temp(temp: &Path, path: &Path) -> CoreResult<()> {
    fs::rename(temp, path).map_err(|error| CoreError::Io(error.to_string()))?;
    if let Some(parent) = path.parent() {
        if let Ok(directory) = File::open(parent) {
            let _ = directory.sync_all();
        }
    }
    Ok(())
}

pub fn save(path: &Path, store: &NativeStore) -> CoreResult<()> {
    let temp = write_temp(path, store)?;
    commit_temp(&temp, path)
}
