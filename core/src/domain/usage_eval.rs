use chrono::Datelike;
use serde::Serialize;

use crate::clock::Moment;
use crate::models::{Period, UsageLimit};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UsageState {
    pub period_key: String,
    pub used_minutes: u32,
    pub sessions: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct UsageBudgetStatus {
    pub kind: String,
    pub period_key: String,
    pub used_seconds: u64,
    pub used_minutes: u32,
    pub limit_minutes: Option<u32>,
    pub remaining_seconds: Option<u64>,
    pub sessions_used: u32,
    pub max_sessions: Option<u32>,
    pub remaining_sessions: Option<u32>,
    pub session_open: bool,
}

pub fn period_key(period: Period, moment: &Moment) -> String {
    let local = moment.utc.with_timezone(&moment.zone);
    match period {
        Period::Daily => format!("{:04}-{:02}-{:02}", local.year(), local.month(), local.day()),
        Period::Weekly => format!("{:04}-W{:02}", local.year(), local.iso_week().week()),
        Period::Monthly => format!("{:04}-{:02}", local.year(), local.month()),
    }
}

pub fn usage_allows(usage: Option<&UsageLimit>, state: &UsageState, moment: &Moment) -> bool {
    usage_allows_tracked(usage, state, false, moment)
}

pub fn usage_allows_tracked(
    usage: Option<&UsageLimit>,
    state: &UsageState,
    session_open: bool,
    moment: &Moment,
) -> bool {
    match usage {
        None => false,
        Some(UsageLimit::TimeLimit { minutes, period }) => {
            let key = period_key(*period, moment);
            let used = if state.period_key == key {
                state.used_minutes
            } else {
                0
            };
            used < *minutes
        }
        Some(UsageLimit::SessionLimit { max_sessions, period }) => {
            if *max_sessions == 0 {
                return false;
            }
            let key = period_key(*period, moment);
            let sessions = if state.period_key == key {
                state.sessions
            } else {
                0
            };
            if session_open {
                sessions <= *max_sessions
            } else {
                sessions < *max_sessions
            }
        }
    }
}

pub fn budget_status(
    usage: Option<&UsageLimit>,
    state: &UsageState,
    active_seconds: u64,
    session_open: bool,
    moment: &Moment,
) -> Option<UsageBudgetStatus> {
    match usage? {
        UsageLimit::TimeLimit { minutes, period } => {
            let key = period_key(*period, moment);
            let same = state.period_key == key;
            let used_seconds = if same {
                active_seconds.max((state.used_minutes as u64).saturating_mul(60))
            } else {
                0
            };
            let used_minutes = if same { state.used_minutes } else { 0 };
            let total_seconds = (*minutes as u64).saturating_mul(60);
            Some(UsageBudgetStatus {
                kind: "Time Limit".to_string(),
                period_key: key,
                used_seconds,
                used_minutes,
                limit_minutes: Some(*minutes),
                remaining_seconds: Some(total_seconds.saturating_sub(used_seconds)),
                sessions_used: if same { state.sessions } else { 0 },
                max_sessions: None,
                remaining_sessions: None,
                session_open: same && session_open,
            })
        }
        UsageLimit::SessionLimit { max_sessions, period } => {
            let key = period_key(*period, moment);
            let same = state.period_key == key;
            let sessions_used = if same { state.sessions } else { 0 };
            Some(UsageBudgetStatus {
                kind: "Session Limit".to_string(),
                period_key: key,
                used_seconds: if same { active_seconds } else { 0 },
                used_minutes: if same { state.used_minutes } else { 0 },
                limit_minutes: None,
                remaining_seconds: None,
                sessions_used,
                max_sessions: Some(*max_sessions),
                remaining_sessions: Some(max_sessions.saturating_sub(sessions_used)),
                session_open: same && session_open,
            })
        }
    }
}
