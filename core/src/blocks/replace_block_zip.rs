//! Replaces a block's JSON document inside a settings zip, re-packages the zip, and verifies the
//! result with [`check_zip`]. The input bytes are never modified.

use crate::io::zip_check::{check_zip, ZipVerdict};
use crate::io::zip_reader::open_zip;
use crate::io::zip_writer::write_zip;
use serde_json::Value;
use std::collections::HashSet;

/// Replaces `path` in `bytes` with pretty-printed `new_doc`, writes the zip, and checks it.
pub(crate) fn replace_block_document_zip(bytes: &[u8], path: &str, new_doc: &Value) -> Result<Vec<u8>, String> {
    let mut files = open_zip(bytes).map_err(|why| format!("The settings file was refused: {why}"))?.files;
    let entry = files.iter_mut().find(|(name, _)| name == path).ok_or("The block file is missing.")?;
    let mut text = serde_json::to_string_pretty(new_doc).map_err(|e| e.to_string())?;
    text.push('\n');
    entry.1 = text.into_bytes();
    let result = write_zip(&files).map_err(|why| format!("The edited zip could not be made: {why}"))?;
    match check_zip(&result, &HashSet::new()) {
        ZipVerdict::Read(_) => Ok(result),
        ZipVerdict::Refused(reason) => Err(format!("The edited settings were refused: {}", reason.sentence)),
    }
}
