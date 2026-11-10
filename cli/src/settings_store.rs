//! Stores the user's settings archive outside the source checkout.
use crate::delayed_changes::waiting_file_disk::{read_waiting_changes, waiting_path};
use crate::status::settings_file::read_settings_and_bytes;
use leko_core::io::zip_check::{check_zip, ZipVerdict};
use leko_core::io::zip_writer::write_zip;
use std::ffi::OsString;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);
const GUIDE: &[u8] = b"Leko settings created for a new local setup.\n";
const GENERAL: &[u8] = br#"{"schema":2}"#;

pub fn default_path(home: Option<&std::ffi::OsStr>) -> Result<PathBuf, String> {
    let home = Path::new(home.ok_or("The home folder is unavailable.")?);
    if !home.is_absolute() {
        return Err("The home folder must be an absolute path.".into());
    }
    #[cfg(target_os = "macos")]
    let base = home.join("Library/Application Support/Leko");
    #[cfg(target_os = "windows")]
    let base = home.join("AppData/Roaming/Leko");
    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    let base = home.join(".local/share/Leko");
    let path = base.join("settings.zip");
    if !path.is_absolute() {
        return Err("The settings path must be absolute.".into());
    }
    Ok(path)
}

fn private_directory(path: &Path) -> Result<(), String> {
    fs::create_dir_all(path)
        .map_err(|error| format!("Could not create the settings folder: {error}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))
            .map_err(|error| format!("Could not protect the settings folder: {error}"))?;
    }
    Ok(())
}

fn write_new(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or("The settings path has no parent folder.")?;
    private_directory(parent)?;
    let mut temp = None;
    for _ in 0..16 {
        let id = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
        let candidate = parent.join(format!(".settings-{id}.tmp"));
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(file) => {
                temp = Some((candidate, file));
                break;
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(format!(
                    "Could not create a temporary settings file: {error}"
                ))
            }
        }
    }
    let (temp_path, mut file) = temp.ok_or("Could not reserve a temporary settings file.")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Err(error) = file.set_permissions(fs::Permissions::from_mode(0o600)) {
            let _ = fs::remove_file(&temp_path);
            return Err(format!("Could not protect the settings file: {error}"));
        }
    }
    if let Err(error) = file.write_all(bytes).and_then(|()| file.sync_all()) {
        let _ = fs::remove_file(&temp_path);
        return Err(format!("Could not write the settings file: {error}"));
    }
    drop(file);
    let result = fs::hard_link(&temp_path, path);
    let _ = fs::remove_file(&temp_path);
    match result {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            Err("Settings already exist; the existing file was left unchanged.".into())
        }
        Err(error) => Err(format!("Could not save the settings file: {error}")),
    }
}

fn empty_archive() -> Result<Vec<u8>, String> {
    let files = vec![
        ("GUIDE.md".to_string(), GUIDE.to_vec()),
        ("general.json".to_string(), GENERAL.to_vec()),
    ];
    let bytes = write_zip(&files)?;
    match check_zip(&bytes, &std::collections::HashSet::new()) {
        ZipVerdict::Read(_) => Ok(bytes),
        ZipVerdict::Refused(refusal) => Err(refusal.sentence),
    }
}

fn validate(path: &Path) -> Result<(), String> {
    let name = path
        .to_str()
        .ok_or("The settings path is not valid text.")?;
    read_settings_and_bytes(name)
        .map(|_| ())
        .and_then(|()| read_waiting_changes(&waiting_path(name)).map(|_| ()))
}

fn initialize(path: &Path) -> Result<(), String> {
    if path.exists() {
        validate(path)
            .map_err(|error| format!("Settings already exist but could not be read: {error}"))?;
        return Ok(());
    }
    let waiting = waiting_path(
        path.to_str()
            .ok_or("The settings path is not valid text.")?,
    );
    if Path::new(&waiting).exists() {
        return Err(
            "A pending-changes file exists without its settings archive; it was left untouched."
                .into(),
        );
    }
    write_new(path, &empty_archive()?)?;
    validate(path).map_err(|error| format!("The new settings file could not be verified: {error}"))
}

fn import(path: &Path, source: &Path) -> Result<(), String> {
    if source == path {
        return validate(path);
    }
    let source_text = source
        .to_str()
        .ok_or("The import path is not valid text.")?;
    let (bytes, _) = read_settings_and_bytes(source_text)?;
    let source_waiting = waiting_path(source_text);
    let waiting_bytes = match read_waiting_changes(&source_waiting)? {
        Some(_) => Some(
            fs::read(&source_waiting)
                .map_err(|error| format!("Could not read pending changes: {error}"))?,
        ),
        None => None,
    };
    let destination_waiting = waiting_path(
        path.to_str()
            .ok_or("The settings path is not valid text.")?,
    );
    if Path::new(&destination_waiting).exists() {
        return Err(
            "Pending changes already exist at the new settings location; they were left untouched."
                .into(),
        );
    }
    write_new(path, &bytes)?;
    if let Some(waiting_bytes) = waiting_bytes {
        if let Err(error) = write_new(Path::new(&destination_waiting), &waiting_bytes) {
            let _ = fs::remove_file(path);
            return Err(format!(
                "Could not migrate pending changes; the new settings copy was rolled back: {error}"
            ));
        }
    }
    validate(path).map_err(|error| {
        let _ = fs::remove_file(path);
        let _ = fs::remove_file(&destination_waiting);
        format!("The imported settings could not be verified: {error}")
    })
}

/// Runs `leko settings path|validate|init|import` with an injectable home for tests.
pub fn run(
    args: &[OsString],
    home: Option<&std::ffi::OsStr>,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    let result = (|| -> Result<String, String> {
        let words: Vec<&str> = args
            .iter()
            .map(|arg| arg.to_str().ok_or("Arguments must be valid text."))
            .collect::<Result<_, _>>()?;
        let path = default_path(home)?;
        match words.as_slice() {
            ["path"] => Ok(path.display().to_string()),
            ["validate", target] => {
                validate(Path::new(target))?;
                Ok("Settings are valid.".into())
            }
            ["init"] => {
                initialize(&path)?;
                Ok(format!("Settings are ready at {}.", path.display()))
            }
            ["import", source] => {
                import(&path, Path::new(source))?;
                Ok(format!("Settings were imported to {}.", path.display()))
            }
            _ => Err(
                "Usage: leko settings path | validate FILE | init | import EXPORTED_FILE".into(),
            ),
        }
    })();
    match result {
        Ok(text) => {
            let _ = writeln!(out, "{text}");
            0
        }
        Err(problem) => {
            let _ = writeln!(err, "{problem}");
            1
        }
    }
}
