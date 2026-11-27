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

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct CountdownItem {
    pub rule_id: String,
    pub rule: String,
    pub kind: String,
    pub transition: String,
    pub remaining_seconds: u64,
    pub transitions_at: String,
    #[serde(skip_serializing)]
    pub urgency: u8,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct CountdownsReport {
    pub at: String,
    pub countdowns: Vec<CountdownItem>,
    pub soonest: Option<CountdownItem>,
}

fn pass_expiry(
    earned: chrono::DateTime<chrono::Utc>,
    length: &crate::models::PassLength,
    moment: &Moment,
) -> Option<chrono::DateTime<chrono::Utc>> {
    use chrono::{Duration, LocalResult, TimeZone, Utc};
    match length {
        crate::models::PassLength::TimedPass { minutes } => {
            Some(earned + Duration::minutes(*minutes as i64))
        }
        crate::models::PassLength::RestOfDayPass => {
            let local = earned.with_timezone(&moment.zone);
            let next_midnight = (local.date_naive() + Duration::days(1)).and_hms_opt(0, 0, 0)?;
            match moment.zone.from_local_datetime(&next_midnight) {
                LocalResult::Single(value) => Some(value.with_timezone(&Utc)),
                LocalResult::Ambiguous(earliest, _) => Some(earliest.with_timezone(&Utc)),
                LocalResult::None => None,
            }
        }
    }
}

pub fn rule_countdowns(
    rule: &crate::models::TechnologyManagementRule,
    usage_file: &crate::services::UsageStateFile,
    pass_file: &crate::services::PassStateFile,
    moment: &Moment,
) -> Vec<CountdownItem> {
    use crate::domain::{is_time_active, pass_active, schedule_eval::schedule_transition};
    use crate::models::{Pass, PassLength, PassRequirement, Seal};
    use crate::services::{domain_state, domain_usage};
    use chrono::Duration;

    let mut items = Vec::new();
    let p_state = domain_state(pass_file, &rule.id);
    let p_active = pass_active(&rule.pass, &p_state, moment);
    let time_active = is_time_active(&rule.schedule.time_based, moment);

    if rule.enabled && time_active && !p_active {
        if let Some(UsageLimit::TimeLimit { minutes, period }) = &rule.schedule.usage_based {
            let (u_state, active_seconds, _) = domain_usage(usage_file, &rule.id);
            let key = period_key(*period, moment);
            let used = if u_state.period_key == key {
                active_seconds.max((u_state.used_minutes as u64).saturating_mul(60))
            } else {
                0
            };
            let total = (*minutes as u64).saturating_mul(60);
            if used < total {
                let rem = total - used;
                items.push(CountdownItem {
                    rule_id: rule.id.clone(), rule: rule.name.clone(),
                    kind: "Time Limit".into(), transition: "Exhausts".into(),
                    remaining_seconds: rem,
                    transitions_at: (moment.utc + Duration::seconds(rem as i64)).to_rfc3339(),
                    urgency: 0,
                });
            }
        }
    }
    if rule.enabled && p_active {
        if let Pass::EarnPass { length, .. } = &rule.pass {
            if let Some(end) = p_state.earned_at.as_deref()
                .and_then(|s| Moment::parse(s, moment.zone).ok())
                .and_then(|earned| pass_expiry(earned.utc, length, moment))
            {
                if end > moment.utc {
                    let kind = match length {
                        PassLength::TimedPass { .. } => "Timed Pass",
                        PassLength::RestOfDayPass => "Rest Of Day Pass",
                    };
                    items.push(CountdownItem {
                        rule_id: rule.id.clone(), rule: rule.name.clone(),
                        kind: kind.into(), transition: "Expires".into(),
                        remaining_seconds: (end - moment.utc).num_seconds() as u64,
                        transitions_at: end.to_rfc3339(), urgency: 1,
                    });
                }
            }
        }
    }
    if rule.enabled && time_active && !p_active {
        if let Pass::EarnPass {
            requirement: PassRequirement::EngagementPass { required_seconds, .. }, ..
        } = &rule.pass {
            if let Some(entry) = usage_file.rules.get(&rule.id) {
                let eng_key = period_key(Period::Daily, moment);
                let same = entry.engagement_window_key == eng_key;
                let earned = if same { entry.engagement_seconds } else { 0 };
                if same && (earned > 0 || entry.last_engagement_at.is_some()) && earned < *required_seconds {
                    let rem = *required_seconds - earned;
                    items.push(CountdownItem {
                        rule_id: rule.id.clone(), rule: rule.name.clone(),
                        kind: "Engagement Pass".into(), transition: "Earns".into(),
                        remaining_seconds: rem,
                        transitions_at: (moment.utc + Duration::seconds(rem as i64)).to_rfc3339(),
                        urgency: 2,
                    });
                }
            }
        }
    }
    if rule.enabled {
        if let Some((kind, tr, at)) = schedule_transition(&rule.schedule.time_based, moment) {
            if at > moment.utc {
                items.push(CountdownItem {
                    rule_id: rule.id.clone(), rule: rule.name.clone(),
                    kind: kind.into(), transition: tr.into(),
                    remaining_seconds: (at - moment.utc).num_seconds() as u64,
                    transitions_at: at.to_rfc3339(), urgency: 3,
                });
            }
        }
    }
    if let Seal::QueuedDelaySeal { delay_seconds, pending_edits } = &rule.seal {
        for edit in pending_edits.iter().filter(|e| !e.cancelled) {
            if let Ok(req) = Moment::parse(&edit.requested_at, moment.zone) {
                let due = req.utc + Duration::seconds(*delay_seconds as i64);
                if due > moment.utc {
                    items.push(CountdownItem {
                        rule_id: rule.id.clone(), rule: rule.name.clone(),
                        kind: "Queued Delay Seal".into(), transition: "Matures".into(),
                        remaining_seconds: (due - moment.utc).num_seconds() as u64,
                        transitions_at: due.to_rfc3339(), urgency: 4,
                    });
                }
            }
        }
    }
    sort_countdowns(&mut items);
    items
}

pub fn sort_countdowns(items: &mut [CountdownItem]) {
    items.sort_by(|a, b| {
        a.remaining_seconds.cmp(&b.remaining_seconds)
            .then_with(|| a.urgency.cmp(&b.urgency))
            .then_with(|| a.rule_id.cmp(&b.rule_id))
            .then_with(|| a.kind.cmp(&b.kind))
    });
}

pub fn compute_countdowns(
    rules: &[crate::models::TechnologyManagementRule],
    usage_file: &crate::services::UsageStateFile,
    pass_file: &crate::services::PassStateFile,
    moment: &Moment,
) -> CountdownsReport {
    let mut countdowns = Vec::new();
    for rule in rules {
        countdowns.extend(rule_countdowns(rule, usage_file, pass_file, moment));
    }
    sort_countdowns(&mut countdowns);
    let soonest = countdowns.first().cloned();
    CountdownsReport { at: moment.utc.to_rfc3339(), countdowns, soonest }
}
