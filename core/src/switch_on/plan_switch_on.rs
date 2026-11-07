//! What switching one block on means, decided from the settings alone. Switching on is a tightening
//! (the loosening judge finds nothing that loosens in `"on": false` becoming `"on": true`), so no lock,
//! key or moment is looked at: the answer is that there is no such block, that it is already on, or
//! that the change lands now. Nothing here reads the clock or a file, and nothing is written.

use crate::models::specs::ReadSet;

/// The answer to "switch this block on".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwitchOnPlan {
    /// The settings hold no block with this id.
    NoSuchBlock,
    /// The block is already on (an `on` of true, or no `on` field, which the phone reads as on).
    AlreadyOn,
    /// The block is off and nothing holds the change back.
    LandsNow,
}

/// What switching the block `id` on means for these settings.
pub fn plan_switch_on(set: &ReadSet, id: &str) -> SwitchOnPlan {
    match set.blocks.get(id) {
        None => SwitchOnPlan::NoSuchBlock,
        Some(block) if block.on => SwitchOnPlan::AlreadyOn,
        Some(_) => SwitchOnPlan::LandsNow,
    }
}
