use crate::clock::Moment;
use crate::domain::pass_eval::pass_active;
use crate::domain::schedule_eval::is_time_active;
use crate::domain::usage_eval::usage_allows_tracked;
use crate::domain::{PassState, UsageState};
use crate::models::TechnologyManagementRule;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecisionReason {
    Disabled,
    OutsideSchedule,
    NoPenalty,
    PassActive,
    UsageWithinBudget,
    Enforced,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Decision {
    pub enforced: bool,
    pub reason: DecisionReason,
}

pub fn decide(
    rule: &TechnologyManagementRule,
    moment: &Moment,
    usage: &UsageState,
    pass: &PassState,
) -> Decision {
    decide_tracked(rule, moment, usage, false, pass)
}

pub fn decide_tracked(
    rule: &TechnologyManagementRule,
    moment: &Moment,
    usage: &UsageState,
    session_open: bool,
    pass: &PassState,
) -> Decision {
    if !rule.enabled {
        return Decision {
            enforced: false,
            reason: DecisionReason::Disabled,
        };
    }
    if !is_time_active(&rule.schedule.time_based, moment) {
        return Decision {
            enforced: false,
            reason: DecisionReason::OutsideSchedule,
        };
    }
    if rule.target.is_whole_device() {
        return Decision {
            enforced: true,
            reason: DecisionReason::Enforced,
        };
    }
    if rule.penalty.is_none() {
        return Decision {
            enforced: false,
            reason: DecisionReason::NoPenalty,
        };
    }
    if pass_active(&rule.pass, pass, moment) {
        return Decision {
            enforced: false,
            reason: DecisionReason::PassActive,
        };
    }
    if usage_allows_tracked(rule.schedule.usage_based.as_ref(), usage, session_open, moment) {
        return Decision {
            enforced: false,
            reason: DecisionReason::UsageWithinBudget,
        };
    }
    Decision {
        enforced: true,
        reason: DecisionReason::Enforced,
    }
}
