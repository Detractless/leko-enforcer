//! The bytes of a settings zip with one block's `"on"` flag set to a wanted value: every other field
//! of the block and every other file of the zip keep their names, contents and order. The zip is read
//! first and refused if the core refuses it, and the new zip is read again before it is returned, so a
//! zip that the core would refuse is never handed back. The input is never changed.

use crate::io::zip_check::{check_zip, ZipVerdict};
use crate::io::zip_reader::open_zip;
use crate::io::zip_writer::write_zip;
use serde_json::Value;
use std::collections::HashSet;

pub(crate) fn refused(verdict: ZipVerdict) -> Result<crate::io::zip_check::ReadZip, String> {
    match verdict {
        ZipVerdict::Read(read) => Ok(read),
        ZipVerdict::Refused(refusal) => Err(refusal.sentence),
    }
}

/// `bytes` with the block `id` switched `on` (or off), or the sentence saying why that cannot be done.
pub(crate) fn set_block_flag(bytes: &[u8], id: &str, on: bool) -> Result<Vec<u8>, String> {
    let none = HashSet::new();
    let before = refused(check_zip(bytes, &none)).map_err(|why| format!("The settings file was refused: {why}"))?;
    match before.set.blocks.get(id) {
        None => return Err(format!("There is no block with the id \"{id}\" in this file.")),
        Some(block) if block.on == on => {
            return Err(format!("The block \"{id}\" is already switched {}.", if on { "on" } else { "off" }))
        }
        Some(_) => {}
    }
    let path = format!("blocks/{id}.json");
    let mut files = open_zip(bytes).map_err(|why| format!("The settings file was refused: {why}"))?.files;
    let Some(entry) = files.iter_mut().find(|(name, _)| *name == path) else { return Err(format!("There is no file {path} in this zip.")) };
    let mut block: Value = serde_json::from_slice(&entry.1).map_err(|_| format!("The file {path} is not valid JSON."))?;
    let Some(fields) = block.as_object_mut() else { return Err(format!("The file {path} is not a JSON object.")) };
    fields.insert("on".to_string(), Value::Bool(on));
    let mut text = serde_json::to_string_pretty(&block).map_err(|problem| problem.to_string())?;
    text.push('\n');
    entry.1 = text.into_bytes();
    let state = if on { "on" } else { "off" };
    let written = write_zip(&files).map_err(|why| format!("The settings file with the block switched {state} could not be made: {why}"))?;
    let after = refused(check_zip(&written, &none)).map_err(|why| format!("The settings file with the block switched {state} was refused: {why}"))?;
    if after.set.blocks.get(id).map_or(on, |block| block.on != on) {
        let still = if on { "off" } else { "on" };
        return Err(format!("The block \"{id}\" was still switched {still} after the change, so nothing was saved."));
    }
    Ok(written)
}
