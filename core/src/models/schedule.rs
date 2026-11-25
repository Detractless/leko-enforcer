use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Period {
    Daily,
    Weekly,
    Monthly,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Window {
    pub start_minute: u32,
    pub end_minute: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum TimeBasedSchedule {
    AlwaysOnSchedule,
    OneTimeSchedule { start_at: String, end_at: String },
    RoutineSchedule { days: Vec<u32>, windows: Vec<Window> },
}

impl Default for TimeBasedSchedule {
    fn default() -> Self {
        TimeBasedSchedule::AlwaysOnSchedule
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum UsageLimit {
    TimeLimit { minutes: u32, period: Period },
    SessionLimit { max_sessions: u32, period: Period },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct Schedule {
    #[serde(default)]
    pub time_based: TimeBasedSchedule,
    #[serde(default)]
    pub usage_based: Option<UsageLimit>,
}
