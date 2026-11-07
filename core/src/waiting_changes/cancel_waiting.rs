//! Takes the parked change of one block out of a list of parked changes. Taking a request back is a
//! tightening, so no lock, wait or key is looked at: only the block id is compared, exactly (a prefix,
//! another case or path-shaped text matches nothing). Every other record comes back as it was, in the
//! same order. Nothing here reads a clock or a file.

use crate::delayed_changes::waiting_change::WaitingChange;

/// What cancelling did to a list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cancelled {
    /// The list without the block's record.
    pub remaining: Vec<WaitingChange>,
    /// The record that was taken out, or `None` when the block had none.
    pub removed: Option<WaitingChange>,
}

/// `changes` without the record of the block `block`, and that record when there was one.
pub fn cancel_waiting(changes: &[WaitingChange], block: &str) -> Cancelled {
    let removed = changes.iter().find(|change| change.block == block).cloned();
    let remaining = changes.iter().filter(|change| change.block != block).cloned().collect();
    Cancelled { remaining, removed }
}
