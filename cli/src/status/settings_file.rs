//! Reads the settings zip named on the command line into the core's settings. This is the one file of
//! the command that touches the disk, and it only reads. A file is refused by its size before it is
//! opened (the core reads a zip from memory and refuses more than 64 MB), and again if it grew while
//! it was read. Every refusal is a plain sentence that names the file safely; none of them is an answer.

use crate::session::session_report::shown;
use leko_core::io::zip_check::{check_zip, ZipVerdict};
use leko_core::io::zip_reader::MAX_TOTAL_BYTES;
use leko_core::models::specs::ReadSet;
use std::collections::HashSet;
use std::io::{ErrorKind, Read};

fn too_big(path: &str) -> String {
    format!("The settings file {} is larger than 64 MB, so it was not read.", shown(path))
}

fn unreadable(path: &str, problem: &std::io::Error) -> String {
    if problem.kind() == ErrorKind::NotFound {
        return format!("The settings file {} was not found.", shown(path));
    }
    format!("The settings file {} could not be read ({problem}).", shown(path))
}

/// The bytes of the file at `path`, at most 64 MB of them.
fn read_bytes(path: &str) -> Result<Vec<u8>, String> {
    let facts = std::fs::metadata(path).map_err(|problem| unreadable(path, &problem))?;
    if !facts.is_file() {
        return Err(format!("{} is not a file.", shown(path)));
    }
    if facts.len() > MAX_TOTAL_BYTES as u64 {
        return Err(too_big(path));
    }
    let file = std::fs::File::open(path).map_err(|problem| unreadable(path, &problem))?;
    let mut bytes = Vec::new();
    file.take(MAX_TOTAL_BYTES as u64 + 1).read_to_end(&mut bytes).map_err(|problem| unreadable(path, &problem))?;
    if bytes.len() > MAX_TOTAL_BYTES {
        return Err(too_big(path));
    }
    Ok(bytes)
}

/// The bytes of the settings file at `path`: not empty, and at most 64 MB.
pub fn read_settings_bytes(path: &str) -> Result<Vec<u8>, String> {
    let bytes = read_bytes(path)?;
    if bytes.is_empty() {
        return Err(format!("The settings file {} is empty.", shown(path)));
    }
    Ok(bytes)
}

/// The bytes of the settings zip at `path` and the settings in it, read as the core reads them, or
/// the sentence saying why they could not be read. A zip with no blocks is a real, empty set.
pub fn read_settings_and_bytes(path: &str) -> Result<(Vec<u8>, ReadSet), String> {
    let bytes = read_settings_bytes(path)?;
    match check_zip(&bytes, &HashSet::new()) {
        ZipVerdict::Read(read) => Ok((bytes, read.set)),
        ZipVerdict::Refused(refusal) => Err(format!("The settings file {} was refused: {}", shown(path), refusal.sentence)),
    }
}

/// The settings in the zip at `path`, or the sentence saying why they could not be read.
pub fn read_settings(path: &str) -> Result<ReadSet, String> {
    read_settings_and_bytes(path).map(|(_, set)| set)
}
