use std::collections::HashMap;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::DateTime;
use serde::{Deserialize, Serialize};

use crate::clock::Moment;
use crate::domain::{is_time_active, period_key, UsageState};
use crate::error::{CoreError, CoreResult};
use crate::models::{Pass, PassRequirement, Period, TechnologyManagementRule, UsageLimit};

pub const USAGE_STATE_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UsageStateFile {
    pub version: u32,
    pub rules: HashMap<String, RuleUsage>,
}

impl Default for UsageStateFile {
    fn default() -> Self {
        UsageStateFile {
            version: USAGE_STATE_VERSION,
            rules: HashMap::new(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuleUsage {
    pub rule_id: String,
    pub period_key: String,
    pub active_seconds: u64,
    pub sessions: u32,
    #[serde(default)]
    pub session_open: bool,
    #[serde(default)]
    pub last_active_at: Option<String>,
    #[serde(default)]
    pub away_since: Option<String>,
    #[serde(default)]
    pub engagement_seconds: u64,
    #[serde(default)]
    pub engagement_window_key: String,
    #[serde(default)]
    pub last_engagement_at: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RuleTickOutcome {
    pub grant_engagement_pass: bool,
}

pub fn state_path(store_path: &Path) -> PathBuf {
    let stem = store_path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("rules");
    store_path.with_file_name(format!("{}.usage-state.json", stem))
}

pub fn load(path: &Path) -> CoreResult<UsageStateFile> {
    if !path.exists() {
        return Ok(UsageStateFile::default());
    }
    let bytes = fs::read(path).map_err(|error| CoreError::Io(error.to_string()))?;
    if bytes.is_empty() {
        return Err(CoreError::Parse("the usage state file is empty".to_string()));
    }
    let state: UsageStateFile =
        serde_json::from_slice(&bytes).map_err(|error| CoreError::Parse(error.to_string()))?;
    if state.version != USAGE_STATE_VERSION {
        return Err(CoreError::UnknownVersion(state.version));
    }
    Ok(state)
}

pub fn save(path: &Path, state: &UsageStateFile) -> CoreResult<()> {
    if state.version != USAGE_STATE_VERSION {
        return Err(CoreError::UnknownVersion(state.version));
    }
    let bytes =
        serde_json::to_vec_pretty(state).map_err(|error| CoreError::Parse(error.to_string()))?;
    let temp = path.with_extension("tmp");
    let mut file = File::create(&temp).map_err(|error| CoreError::Io(error.to_string()))?;
    file.write_all(&bytes)
        .map_err(|error| CoreError::Io(error.to_string()))?;
    file.sync_all()
        .map_err(|error| CoreError::Io(error.to_string()))?;
    drop(file);
    fs::rename(&temp, path).map_err(|error| CoreError::Io(error.to_string()))?;
    Ok(())
}

pub fn domain_usage(state: &UsageStateFile, rule_id: &str) -> (UsageState, u64, bool) {
    match state.rules.get(rule_id) {
        Some(entry) => (
            UsageState {
                period_key: entry.period_key.clone(),
                used_minutes: (entry.active_seconds / 60).min(u32::MAX as u64) as u32,
                sessions: entry.sessions,
            },
            entry.active_seconds,
            entry.session_open,
        ),
        None => (
            UsageState {
                period_key: String::new(),
                used_minutes: 0,
                sessions: 0,
            },
            0,
            false,
        ),
    }
}

pub fn budget_period_key(rule: &TechnologyManagementRule, moment: &Moment) -> String {
    match &rule.schedule.usage_based {
        Some(UsageLimit::TimeLimit { period, .. })
        | Some(UsageLimit::SessionLimit { period, .. }) => period_key(*period, moment),
        None => period_key(Period::Daily, moment),
    }
}

fn elapsed_seconds(prev_rfc3339: Option<&str>, moment: &Moment, max_gap_seconds: u64) -> u64 {
    let Some(text) = prev_rfc3339 else {
        return 0;
    };
    let Ok(parsed) = DateTime::parse_from_rfc3339(text) else {
        return 0;
    };
    let delta = moment.utc.signed_duration_since(parsed.with_timezone(&chrono::Utc)).num_seconds();
    if delta <= 0 {
        return 0;
    }
    (delta as u64).min(max_gap_seconds)
}

fn away_expired(away_rfc3339: Option<&str>, moment: &Moment, debounce_seconds: u64) -> bool {
    let Some(text) = away_rfc3339 else {
        return false;
    };
    let Ok(parsed) = DateTime::parse_from_rfc3339(text) else {
        return true;
    };
    let gap = moment.utc.signed_duration_since(parsed.with_timezone(&chrono::Utc)).num_seconds();
    gap >= debounce_seconds as i64
}

pub fn advance_rule_usage(
    state: &mut UsageStateFile,
    rule: &TechnologyManagementRule,
    moment: &Moment,
    target_active: bool,
    engagement_active: bool,
    pass_is_active: bool,
    debounce_seconds: u64,
    max_gap_seconds: u64,
) -> RuleTickOutcome {
    let p_key = budget_period_key(rule, moment);
    let eng_key = period_key(Period::Daily, moment);
    let entry = state.rules.entry(rule.id.clone()).or_insert_with(|| RuleUsage {
        rule_id: rule.id.clone(),
        period_key: p_key.clone(),
        engagement_window_key: eng_key.clone(),
        ..Default::default()
    });

    if entry.period_key != p_key {
        entry.period_key = p_key;
        entry.active_seconds = 0;
        entry.sessions = 0;
        entry.session_open = false;
        entry.last_active_at = None;
        entry.away_since = None;
    }
    if entry.engagement_window_key != eng_key {
        entry.engagement_window_key = eng_key;
        entry.engagement_seconds = 0;
        entry.last_engagement_at = None;
    }

    if !rule.enabled || !is_time_active(&rule.schedule.time_based, moment) {
        entry.session_open = false;
        entry.away_since = None;
        entry.last_active_at = None;
        entry.last_engagement_at = None;
        return RuleTickOutcome::default();
    }

    let now_text = moment.utc.to_rfc3339();
    let mut outcome = RuleTickOutcome::default();

    if !pass_is_active {
        if let Pass::EarnPass {
            requirement: PassRequirement::EngagementPass { required_seconds, .. },
            ..
        } = &rule.pass
        {
            if engagement_active {
                let dt = elapsed_seconds(entry.last_engagement_at.as_deref(), moment, max_gap_seconds);
                entry.engagement_seconds = entry.engagement_seconds.saturating_add(dt);
                entry.last_engagement_at = Some(now_text.clone());
                if entry.engagement_seconds >= *required_seconds {
                    outcome.grant_engagement_pass = true;
                }
            } else {
                entry.last_engagement_at = None;
            }
        }
    }

    if pass_is_active || rule.schedule.usage_based.is_none() {
        if !target_active {
            entry.session_open = false;
            entry.away_since = None;
            entry.last_active_at = None;
        }
        return outcome;
    }

    let session_exhausted = matches!(
        rule.schedule.usage_based,
        Some(UsageLimit::SessionLimit { max_sessions, .. }) if entry.sessions >= max_sessions
    );

    if target_active {
        if !entry.session_open {
            if !session_exhausted {
                entry.sessions = entry.sessions.saturating_add(1);
                entry.session_open = true;
                entry.away_since = None;
                entry.last_active_at = Some(now_text);
            }
        } else if entry.away_since.is_some() {
            if away_expired(entry.away_since.as_deref(), moment, debounce_seconds) {
                if session_exhausted {
                    entry.session_open = false;
                    entry.away_since = None;
                    entry.last_active_at = None;
                } else {
                    entry.sessions = entry.sessions.saturating_add(1);
                    entry.session_open = true;
                    entry.away_since = None;
                    entry.last_active_at = Some(now_text);
                }
            } else {
                entry.away_since = None;
                entry.last_active_at = Some(now_text);
            }
        } else {
            let dt = elapsed_seconds(entry.last_active_at.as_deref(), moment, max_gap_seconds);
            entry.active_seconds = entry.active_seconds.saturating_add(dt);
            entry.last_active_at = Some(now_text);
        }
    } else if entry.session_open {
        if entry.away_since.is_none() {
            entry.away_since = Some(now_text);
        }
        if away_expired(entry.away_since.as_deref(), moment, debounce_seconds) {
            entry.session_open = false;
            entry.away_since = None;
            entry.last_active_at = None;
        }
    }

    outcome
}

pub fn reset_engagement(state: &mut UsageStateFile, rule_id: &str) {
    if let Some(entry) = state.rules.get_mut(rule_id) {
        entry.engagement_seconds = 0;
        entry.last_engagement_at = None;
    }
}
