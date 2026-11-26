use serde::{Deserialize, Serialize};

use crate::models::verifier_spec::CustomVerifierSpec;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum PassRequirement {
    PhysicalPass,
    MentalPass,
    CountdownPass { wait_seconds: u64 },
    TypingPass,
    CustomVerifier(CustomVerifierSpec),
    EngagementPass { targets: Vec<String>, required_seconds: u64 },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum PassLength {
    TimedPass { minutes: u32 },
    RestOfDayPass,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum PassCap {
    LimitedPasses { max_per_window: u32 },
    UnlimitedPasses,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Pass {
    NoPass,
    EarnPass {
        requirement: PassRequirement,
        length: PassLength,
        cap: PassCap,
    },
}

impl Default for Pass {
    fn default() -> Self {
        Pass::NoPass
    }
}
