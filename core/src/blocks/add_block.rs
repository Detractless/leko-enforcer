//! Adds a drafted block to the bytes of a settings zip. The zip is read first and refused if the core
//! refuses it, or if a block with the same id is already in it (on or off); the block is added as its
//! own file and every other file keeps its name and bytes; the new zip is read again before it is
//! returned, in the core's reading and in the phone's. The phone's refusal is not an error: the
//! zip is good for Leko Enforcer, and the reason is returned as a warning. The input is never changed.

use super::draft_block::Draft;
use crate::io::zip_check::{check_zip, check_zip_in_mode, ZipVerdict};
use crate::io::zip_reader::open_zip;
use crate::io::zip_writer::write_zip;
use crate::models::specs::ReadMode;
use std::collections::HashSet;

/// Why a block was not added.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AddProblem {
    /// The zip given is one the core refuses; the core's sentence.
    FileRefused(String),
    /// A block with this id is already in the zip.
    IdTaken(String),
    /// The zip with the block added is one the core refuses or cannot write; the sentence.
    ResultRefused(String),
}

impl AddProblem {
    pub fn sentence(&self) -> String {
        match self {
            AddProblem::FileRefused(why) => format!("The settings file was refused: {why}"),
            AddProblem::IdTaken(id) => format!("A block with the id \"{id}\" is already in this file, so nothing was added. Pick another name."),
            AddProblem::ResultRefused(why) => format!("The settings file with the new block was refused: {why}"),
        }
    }
}

/// The zip with the block in it, and why the phone would not read it (none when it would).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Added {
    pub bytes: Vec<u8>,
    pub phone_warning: Option<String>,
}

/// The path the block is written at inside the zip.
pub fn block_path(id: &str) -> String {
    format!("blocks/{id}.json")
}

/// `bytes` with the `draft` block added as `blocks/<id>.json`, or the reason it was not.
pub fn add_block(bytes: &[u8], draft: &Draft) -> Result<Added, AddProblem> {
    let none = HashSet::new();
    match check_zip(bytes, &none) {
        ZipVerdict::Refused(refusal) => return Err(AddProblem::FileRefused(refusal.sentence)),
        ZipVerdict::Read(read) if read.set.blocks.contains(&draft.id) => return Err(AddProblem::IdTaken(draft.id.clone())),
        ZipVerdict::Read(_) => {}
    }
    let mut files = open_zip(bytes).map_err(AddProblem::FileRefused)?.files;
    let mut text = serde_json::to_string_pretty(&draft.block).map_err(|problem| AddProblem::ResultRefused(problem.to_string()))?;
    text.push('\n');
    files.push((block_path(&draft.id), text.into_bytes()));
    files.sort_by(|left, right| left.0.cmp(&right.0));
    let written = write_zip(&files).map_err(AddProblem::ResultRefused)?;
    if let ZipVerdict::Refused(refusal) = check_zip(&written, &none) {
        return Err(AddProblem::ResultRefused(refusal.sentence));
    }
    let phone_warning = match check_zip_in_mode(&written, &none, ReadMode::Phone) {
        ZipVerdict::Refused(refusal) => Some(refusal.sentence),
        ZipVerdict::Read(_) => None,
    };
    Ok(Added { bytes: written, phone_warning })
}
