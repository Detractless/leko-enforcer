//! One parked change: a switch-off of one block, the moment it was asked for and the moment it may
//! land. Both moments are whole seconds and are written as UTC text (YYYY-MM-DDTHH:MM:SSZ), so a
//! zone or a clock change never moves them. A record is made only through [`WaitingChange::new`],
//! which refuses a block id the readers would refuse, a moment the text cannot hold, and a due moment
//! before the request moment.

use crate::config::read_block::is_item_id;
use serde_json::Value;
use crate::sessions::utc_moment::{format_utc_moment, parse_utc_moment};

const SECOND_MS: i64 = 1_000;

/// What a parked change will do when it lands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeKind {
    SwitchOff,
    EditCoverage,
    EditSchedule,
}

impl ChangeKind {
    /// The word the waiting file uses for this kind.
    pub fn word(self) -> &'static str {
        match self {
            ChangeKind::SwitchOff => "switch-off",
            ChangeKind::EditCoverage => "edit-coverage",
            ChangeKind::EditSchedule => "edit-schedule",
        }
    }

    pub fn from_word(word: &str) -> Option<ChangeKind> {
        match word {
            "switch-off" => Some(ChangeKind::SwitchOff),
            "edit-coverage" => Some(ChangeKind::EditCoverage),
            "edit-schedule" => Some(ChangeKind::EditSchedule),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoverageAction { AddApp, RemoveApp, AddSite, RemoveSite }

impl CoverageAction {
    pub fn word(self) -> &'static str {
        match self { Self::AddApp => "add-app", Self::RemoveApp => "remove-app", Self::AddSite => "add-site", Self::RemoveSite => "remove-site" }
    }
    pub fn from_word(word: &str) -> Option<Self> {
        match word { "add-app" => Some(Self::AddApp), "remove-app" => Some(Self::RemoveApp), "add-site" => Some(Self::AddSite), "remove-site" => Some(Self::RemoveSite), _ => None }
    }
    pub fn is_app(self) -> bool { matches!(self, Self::AddApp | Self::RemoveApp) }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoverageOperation { pub action: CoverageAction, pub target: String }

pub const MAX_EDIT_OPERATIONS: usize = 1000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WaitingChange {
    pub kind: ChangeKind,
    pub block: String,
    /// When the change was asked for, in milliseconds since the epoch (a whole second).
    pub requested_at: i64,
    /// The earliest moment it may land, in milliseconds since the epoch (a whole second).
    pub due_at: i64,
    /// Empty for switch-off; edits contain explicit operations with implied before-state preconditions.
    pub operations: Vec<CoverageOperation>,
    pub rule: Option<String>,
    pub before: Option<Value>,
    pub after: Option<Value>,
}

/// The UTC text of a whole-second moment; `None` for a moment the text cannot hold.
pub fn moment_text(ms: i64) -> Option<String> {
    if ms % SECOND_MS != 0 {
        return None;
    }
    format_utc_moment(ms)
}

/// The moment `text` names, or the sentence saying `what` is not a moment.
pub fn read_moment(text: &str, what: &str) -> Result<i64, String> {
    parse_utc_moment(text).ok_or_else(|| format!("{what} must be a UTC moment written like 2026-11-06T10:00:00Z"))
}

impl WaitingChange {
    pub fn new(kind: ChangeKind, block: &str, requested_at: i64, due_at: i64) -> Result<WaitingChange, String> {
        if kind != ChangeKind::SwitchOff { return Err("coverage edits require operations".into()); }
        Self::validate_times(block, requested_at, due_at)?;
        Ok(WaitingChange { kind, block: block.to_string(), requested_at, due_at, operations: Vec::new(), rule: None, before: None, after: None })
    }

    fn validate_times(block: &str, requested_at: i64, due_at: i64) -> Result<(), String> {
        if !is_item_id(block) {
            return Err(format!("the block id {block:?} is not an id a settings file can hold"));
        }
        for (what, moment) in [("requestedAt", requested_at), ("dueAt", due_at)] {
            if moment_text(moment).is_none() {
                return Err(format!("{what} is not a whole-second moment between the years 0 and 9999"));
            }
        }
        if due_at < requested_at {
            return Err("dueAt is before requestedAt".to_string());
        }
        Ok(())
    }

    pub fn edit(block: &str, requested_at: i64, due_at: i64, operations: Vec<CoverageOperation>) -> Result<WaitingChange, String> {
        Self::validate_times(block, requested_at, due_at)?;
        if operations.is_empty() || operations.len() > MAX_EDIT_OPERATIONS { return Err(format!("coverage edits require 1 to {MAX_EDIT_OPERATIONS} operations")); }
        for (index, op) in operations.iter().enumerate() {
            let valid = if op.action.is_app() { !op.target.is_empty() && op.target.len() <= 255 && crate::config::read_block::is_package(&op.target) } else { crate::sites::site_name::is_site(&op.target) };
            if !valid { return Err(format!("operation {} has an invalid target", index + 1)); }
            if operations[..index].iter().any(|previous| previous.action.is_app() == op.action.is_app() && previous.target == op.target) {
                return Err(format!("target {:?} appears more than once", op.target));
            }
        }
        Ok(WaitingChange { kind: ChangeKind::EditCoverage, block: block.to_string(), requested_at, due_at, operations, rule: None, before: None, after: None })
    }
    pub fn schedule(
        block: &str,
        rule: &str,
        before: Value,
        after: Value,
        requested_at: i64,
        due_at: i64,
    ) -> Result<WaitingChange, String> {
        Self::validate_times(block, requested_at, due_at)?;
        if rule.is_empty() || rule.len() > 100 {
            return Err("invalid rule id".to_string());
        }
        if !before.is_object() || !after.is_object() {
            return Err("rule precondition and replacement must be objects".to_string());
        }
        Ok(WaitingChange {
            kind: ChangeKind::EditSchedule,
            block: block.to_string(),
            requested_at,
            due_at,
            operations: Vec::new(),
            rule: Some(rule.to_string()),
            before: Some(before),
            after: Some(after),
        })
    }
}