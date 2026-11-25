use chrono::Datelike;

use crate::clock::Moment;
use crate::models::{Period, UsageLimit};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UsageState {
    pub period_key: String,
    pub used_minutes: u32,
    pub sessions: u32,
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
            let key = period_key(*period, moment);
            let sessions = if state.period_key == key {
                state.sessions
            } else {
                0
            };
            sessions < *max_sessions
        }
    }
}
