//! Apply bounded coverage operations to the current block and validate the resulting zip.
use crate::config::read_block::is_package;
use crate::delayed_changes::waiting_change::{CoverageAction, CoverageOperation};
use crate::io::zip_check::{check_zip, ZipVerdict};
use crate::sites::site_name::is_site;
use serde_json::Value;
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq)]
pub enum CoverageDocument { Changed(Value), AlreadyApplied, Conflict }

#[derive(Debug, Clone, PartialEq)]
pub enum CoverageApply { Changed { bytes: Vec<u8>, old: Value, new: Value }, AlreadyApplied, Conflict }

/// Apply operations only when all their preconditions match, or all desired effects already exist.
pub fn apply_coverage_document(old: &Value, ops: &[CoverageOperation]) -> Result<CoverageDocument, String> {
    if ops.is_empty() || ops.len() > 1000 { return Err("An edit needs 1 to 1000 coverage operations.".into()); }
    let mut new = old.clone();
    let covers = new.get_mut("covers").and_then(Value::as_object_mut).ok_or("The block has no valid coverage object.")?;
    if covers.contains_key("allAppsExcept") { return Err("Editing all-apps-except coverage is not supported.".into()); }
    let mut seen = HashSet::new();
    let mut applied = 0;
    for op in ops {
        let (key, add, valid) = match op.action {
            CoverageAction::AddApp => ("apps", true, op.target.len() <= 255 && is_package(&op.target)),
            CoverageAction::RemoveApp => ("apps", false, op.target.len() <= 255 && is_package(&op.target)),
            CoverageAction::AddSite => ("sites", true, is_site(&op.target)),
            CoverageAction::RemoveSite => ("sites", false, is_site(&op.target)),
        };
        if !valid { return Err(format!("Invalid {key} target {:?}.", op.target)); }
        if !seen.insert((key, &op.target)) { return Err(format!("The same {key} target occurs more than once.")); }
        let present = covers.get(key).and_then(Value::as_array).is_some_and(|items| items.iter().any(|item| item.as_str() == Some(&op.target)));
        if present == add { continue; }
        applied += 1;
        if add {
            let list = covers.entry(key).or_insert_with(|| Value::Array(Vec::new())).as_array_mut().ok_or("Coverage must be a list.")?;
            list.push(Value::String(op.target.clone()));
        } else {
            let list = covers.get_mut(key).and_then(Value::as_array_mut).ok_or("Coverage must be a list.")?;
            list.retain(|item| item.as_str() != Some(&op.target));
            if list.is_empty() { covers.shift_remove(key); }
        }
    }
    if applied == 0 { return Ok(CoverageDocument::AlreadyApplied); }
    if applied != ops.len() { return Ok(CoverageDocument::Conflict); }
    Ok(CoverageDocument::Changed(new))
}

pub fn apply_coverage_zip(bytes: &[u8], id: &str, ops: &[CoverageOperation]) -> Result<CoverageApply, String> {
    let before = match check_zip(bytes, &HashSet::new()) {
        ZipVerdict::Read(read) => read,
        ZipVerdict::Refused(reason) => return Err(format!("The settings file was refused: {}", reason.sentence)),
    };
    if before.set.blocks.get(id).is_none() { return Err(format!("There is no block with id {id:?}.")); }
    let path = format!("blocks/{id}.json");
    let old = before.set.docs.iter().find(|(name, _)| name == &path).map(|(_, doc)| doc.clone()).ok_or("The block file is missing.")?;
    let new = match apply_coverage_document(&old, ops)? {
        CoverageDocument::AlreadyApplied => return Ok(CoverageApply::AlreadyApplied),
        CoverageDocument::Conflict => return Ok(CoverageApply::Conflict),
        CoverageDocument::Changed(new) => new,
    };
    let result = crate::blocks::replace_block_zip::replace_block_document_zip(bytes, &path, &new)?;
    Ok(CoverageApply::Changed { bytes: result, old, new })
}
