//! Replaces a file with new bytes in one step: the bytes are written to a temporary file in the same
//! folder, flushed to the disk, given the old file's permission bits, and moved over the old file.
//! If any step fails the temporary file is removed and the old file is as it was, byte for byte. A
//! link is followed: the file it points to is replaced and the link stays. This is the only file of
//! the command that writes to the disk. The steps that can fail on a real disk (the write, the flush
//! and the move) go through `Steps`, so a test can make any of them fail. A file that does not exist
//! yet can be created the same way (the waiting file): it is made in one step too, with mode 644.

use crate::session::session_report::shown;
use std::fs::{self, File, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};

const TEMP_TRIES: u32 = 10_000;

/// The words a failed save is told in: what could not be saved and the kind of file left as it was.
#[derive(Debug, Clone, Copy)]
pub struct Words {
    pub subject: &'static str,
    pub file: &'static str,
}

/// A new block in the settings file.
pub const BLOCK_WORDS: Words = Words { subject: "The block", file: "settings file" };
/// A change landed in the settings file.
pub const SETTINGS_WORDS: Words = Words { subject: "The change", file: "settings file" };
/// A parked change in the waiting file.
pub const WAITING_WORDS: Words = Words { subject: "The waiting change", file: "waiting file" };

/// The steps of a replacement; the defaults do the real thing.
pub trait Steps {
    fn write(&mut self, file: &mut File, bytes: &[u8]) -> std::io::Result<()> {
        file.write_all(bytes)
    }

    fn flush(&mut self, file: &File) -> std::io::Result<()> {
        file.sync_all()
    }

    fn rename(&mut self, from: &Path, to: &Path) -> std::io::Result<()> {
        fs::rename(from, to)
    }
}

/// The real disk.
pub struct RealSteps;

impl Steps for RealSteps {}

/// A new, empty temporary file beside `target`, named `.NAME.leko-tmp-N` with the first free N. It is
/// created readable by its owner only; the old file's permissions are copied on before the move.
fn make_temp(target: &Path) -> std::io::Result<(PathBuf, File)> {
    let name = target.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
    let folder = target.parent().filter(|folder| !folder.as_os_str().is_empty()).unwrap_or_else(|| Path::new("."));
    for number in 0..TEMP_TRIES {
        let path = folder.join(format!(".{name}.leko-tmp-{number}"));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
        match options.open(&path) {
            Ok(file) => return Ok((path, file)),
            Err(problem) if problem.kind() == ErrorKind::AlreadyExists => continue,
            Err(problem) => return Err(problem),
        }
    }
    Err(std::io::Error::new(ErrorKind::AlreadyExists, "every temporary file name beside it is taken"))
}

fn unsaved(words: Words, path: &str, step: &str, problem: &std::io::Error) -> String {
    format!("{} could not be saved: {step} ({problem}). The {} {} was left as it was.", words.subject, words.file, shown(path))
}

/// The file to replace: what `path` leads to; when it is not there and `create` is set, its name in
/// its folder (a link that leads nowhere is not "not there": it is left alone).
fn target_of(path: &str, create: bool) -> std::io::Result<PathBuf> {
    match fs::canonicalize(path) {
        Err(problem) if create && problem.kind() == ErrorKind::NotFound && fs::symlink_metadata(path).is_err() => {
            let given = Path::new(path);
            let Some(name) = given.file_name() else { return Err(problem) };
            let folder = given.parent().filter(|folder| !folder.as_os_str().is_empty()).unwrap_or_else(|| Path::new("."));
            Ok(fs::canonicalize(folder)?.join(name))
        }
        found => found,
    }
}

#[cfg(unix)]
fn new_file_permissions() -> Option<fs::Permissions> {
    Some(std::os::unix::fs::PermissionsExt::from_mode(0o644))
}

#[cfg(not(unix))]
fn new_file_permissions() -> Option<fs::Permissions> {
    None
}

/// The permission bits the new file takes: the old file's, or for a file made now the usual ones.
fn permissions_of(target: &Path, create: bool) -> std::io::Result<Option<fs::Permissions>> {
    match fs::metadata(target) {
        Ok(facts) => Ok(Some(facts.permissions())),
        Err(problem) if create && problem.kind() == ErrorKind::NotFound => Ok(new_file_permissions()),
        Err(problem) => Err(problem),
    }
}

/// Replaces the file at `path` (or the file the link at `path` points to) with `bytes`, or gives the
/// sentence saying which step failed. The old file is never changed unless the whole save worked.
pub fn replace_file(path: &str, bytes: &[u8], steps: &mut dyn Steps) -> Result<(), String> {
    save_file(path, bytes, steps, BLOCK_WORDS, false)
}

/// Like [`replace_file`], told in `words`; with `create` set a file that is not there yet is made.
pub fn save_file(path: &str, bytes: &[u8], steps: &mut dyn Steps, words: Words, create: bool) -> Result<(), String> {
    let target = target_of(path, create).map_err(|problem| unsaved(words, path, "the file could not be found again", &problem))?;
    let permissions = permissions_of(&target, create).map_err(|problem| unsaved(words, path, "its permissions could not be read", &problem))?;
    let (temp, mut file) = make_temp(&target).map_err(|problem| unsaved(words, path, "a temporary file could not be made beside it", &problem))?;
    let written = steps
        .write(&mut file, bytes)
        .map_err(|problem| unsaved(words, path, "the new file could not be written", &problem))
        .and_then(|()| steps.flush(&file).map_err(|problem| unsaved(words, path, "the new file could not be flushed to the disk", &problem)))
        .and_then(|()| match permissions {
            Some(bits) => fs::set_permissions(&temp, bits).map_err(|problem| unsaved(words, path, "the permissions could not be copied", &problem)),
            None => Ok(()),
        });
    drop(file);
    let moved = written.and_then(|()| steps.rename(&temp, &target).map_err(|problem| unsaved(words, path, "the new file could not be moved into place", &problem)));
    if moved.is_err() {
        let _ = fs::remove_file(&temp);
    }
    moved
}
