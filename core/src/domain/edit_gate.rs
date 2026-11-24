use crate::clock::Moment;
use crate::domain::seal_eval::seal_satisfied;
use crate::domain::SealProof;
use crate::models::{PassCap, Seal, TechnologyManagementRule};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChangeDirection {
    Tighten,
    Weaken,
    Pause,
    Delete,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProposedEdit {
    pub field: String,
    pub direction: ChangeDirection,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditDecision {
    Allowed,
    Blocked,
    Queued,
}

pub fn allow_edit(
    rule: &TechnologyManagementRule,
    edit: &ProposedEdit,
    moment: &Moment,
    proof: &SealProof,
) -> EditDecision {
    if edit.direction == ChangeDirection::Tighten {
        return EditDecision::Allowed;
    }
    if seal_satisfied(&rule.seal, moment, proof) {
        return EditDecision::Allowed;
    }
    if matches!(rule.seal, Seal::QueuedDelaySeal { .. }) {
        return EditDecision::Queued;
    }
    EditDecision::Blocked
}

pub fn direction_of(old: &TechnologyManagementRule, new: &TechnologyManagementRule) -> ChangeDirection {
    if old.enabled && !new.enabled {
        return ChangeDirection::Pause;
    }
    if old.id != new.id {
        return ChangeDirection::Delete;
    }
    if old.penalty.is_some() && new.penalty.is_none() {
        return ChangeDirection::Weaken;
    }
    match (&old.pass, &new.pass) {
        (
            crate::models::Pass::EarnPass { cap: old_cap, .. },
            crate::models::Pass::EarnPass { cap: new_cap, .. },
        ) => match (old_cap, new_cap) {
            (PassCap::LimitedPasses { max_per_window: old_max }, PassCap::LimitedPasses { max_per_window: new_max }) => {
                if new_max <= old_max {
                    ChangeDirection::Tighten
                } else {
                    ChangeDirection::Weaken
                }
            }
            (PassCap::UnlimitedPasses, PassCap::LimitedPasses { .. }) => ChangeDirection::Tighten,
            (PassCap::LimitedPasses { .. }, PassCap::UnlimitedPasses) => ChangeDirection::Weaken,
            _ => ChangeDirection::Tighten,
        },
        _ => ChangeDirection::Weaken,
    }
}
