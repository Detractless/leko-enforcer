//! Applies schedule changes to an existing outsideHours rule and validates the resulting zip.
use crate::config_time::stretch_overlap::stretches_overlap;
use crate::io::zip_check::{check_zip, ZipVerdict};
use crate::io::zip_reader::open_zip;
use crate::io::zip_writer::write_zip;
use serde_json::{json, Value};
use std::collections::HashSet;

pub const WEEKDAYS: [&str; 7] = ["mon", "tue", "wed", "thu", "fri", "sat", "sun"];

#[derive(Debug, Clone, PartialEq)]
pub enum ScheduleDocument {
    Changed(Value),
    AlreadyApplied,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ScheduleApply {
    Changed { bytes: Vec<u8>, old: Value, new: Value },
    AlreadyApplied,
}

pub fn format_minute(minute: i64) -> String {
    format!("{:02}:{:02}", minute / 60, minute % 60)
}

pub fn validate_days(days: &[String]) -> Result<Vec<String>, String> {
    for day in days {
        if !WEEKDAYS.contains(&day.as_str()) {
            return Err(format!("\"{day}\" is not a valid weekday. Use mon, tue, wed, thu, fri, sat, or sun."));
        }
    }
    let mut seen = HashSet::new();
    for day in days {
        if !seen.insert(day) {
            return Err(format!("Weekday \"{day}\" is named more than once."));
        }
    }
    let ordered: Vec<String> = WEEKDAYS.iter().filter(|d| days.iter().any(|named| named == *d)).map(|d| d.to_string()).collect();
    Ok(ordered)
}

pub fn validate_stretches(stretches: &[(i64, i64)]) -> Result<(), String> {
    if stretches.is_empty() {
        return Err("Allowed hours need at least one stretch.".to_string());
    }
    if stretches.len() > 100 {
        return Err("Too many allowed stretches.".to_string());
    }
    for &(from, to) in stretches {
        if from < 0 || from >= 1440 {
            return Err(format!("Start minute {from} is out of range 00:00..23:59."));
        }
        if to < 0 || to > 1440 {
            return Err(format!("End minute {to} is out of range 00:00..24:00."));
        }
        if from == to {
            return Err(format!("Stretch {from}..{to} has equal start and end."));
        }
    }
    if stretches_overlap(stretches) {
        return Err("Allowed stretches overlap each other.".to_string());
    }
    Ok(())
}

pub fn apply_schedule_document(
    old: &Value,
    rule_id: &str,
    days: Option<&[String]>,
    stretches: Option<&[(i64, i64)]>,
) -> Result<ScheduleDocument, String> {
    let mut new = old.clone();
    let rules = new.get_mut("rules").and_then(Value::as_array_mut).ok_or("The block has no valid rules array.")?;
    let target_idx = rules.iter().position(|r| r.get("id").and_then(Value::as_str) == Some(rule_id))
        .ok_or_else(|| format!("Rule {rule_id:?} not found in block."))?;
    let target = rules[target_idx].as_object_mut().ok_or("Rule is not a JSON object.")?;
    if target.get("kind").and_then(Value::as_str) != Some("outsideHours") {
        return Err(format!("Rule {rule_id:?} is not an outsideHours rule."));
    }

    let mut changed = false;

    if let Some(d) = days {
        let valid_days = validate_days(d)?;
        let old_days = target.get("days").and_then(Value::as_array).map(|arr| {
            arr.iter().filter_map(Value::as_str).map(str::to_string).collect::<Vec<_>>()
        });
        if valid_days.is_empty() || valid_days.len() == 7 {
            if old_days.is_some() {
                target.remove("days");
                changed = true;
            }
        } else {
            let matches = old_days.as_ref().map_or(false, |od| od == &valid_days);
            if !matches {
                target.insert("days".to_string(), json!(valid_days));
                changed = true;
            }
        }
    }

    if let Some(s) = stretches {
        validate_stretches(s)?;
        let new_allowed_json = Value::Array(
            s.iter().map(|&(from, to)| {
                json!({
                    "from": format_minute(from),
                    "to": format_minute(to)
                })
            }).collect()
        );
        let old_allowed = target.get("allowed");
        if old_allowed != Some(&new_allowed_json) {
            target.insert("allowed".to_string(), new_allowed_json);
            changed = true;
        }
    }

    if !changed {
        return Ok(ScheduleDocument::AlreadyApplied);
    }
    Ok(ScheduleDocument::Changed(new))
}

pub fn apply_schedule_zip(
    bytes: &[u8],
    block_id: &str,
    rule_id: &str,
    days: Option<&[String]>,
    stretches: Option<&[(i64, i64)]>,
) -> Result<ScheduleApply, String> {
    let before = match check_zip(bytes, &HashSet::new()) {
        ZipVerdict::Read(read) => read,
        ZipVerdict::Refused(reason) => return Err(format!("The settings file was refused: {}", reason.sentence)),
    };
    if before.set.blocks.get(block_id).is_none() {
        return Err(format!("There is no block with id {block_id:?}."));
    }
    let path = format!("blocks/{block_id}.json");
    let old = before.set.docs.iter().find(|(name, _)| name == &path).map(|(_, doc)| doc.clone())
        .ok_or("The block file is missing.")?;
    let new = match apply_schedule_document(&old, rule_id, days, stretches)? {
        ScheduleDocument::AlreadyApplied => return Ok(ScheduleApply::AlreadyApplied),
        ScheduleDocument::Changed(new) => new,
    };
    let mut files = open_zip(bytes).map_err(|why| format!("The settings file was refused: {why}"))?.files;
    let entry = files.iter_mut().find(|(name, _)| name == &path).ok_or("The block file is missing.")?;
    let mut text = serde_json::to_string_pretty(&new).map_err(|e| e.to_string())?;
    text.push('\n');
    entry.1 = text.into_bytes();
    let result = write_zip(&files).map_err(|why| format!("The edited zip could not be made: {why}"))?;
    match check_zip(&result, &HashSet::new()) {
        ZipVerdict::Read(_) => Ok(ScheduleApply::Changed { bytes: result, old, new }),
        ZipVerdict::Refused(reason) => Err(format!("The edited settings were refused: {}", reason.sentence)),
    }
}
