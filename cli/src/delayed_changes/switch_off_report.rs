//! The sentences of `leko switch-off`: the draft for a change that lands now or must wait, the saved
//! and parked results, and the sentences for a block that is already off or already waiting. Moments
//! are shown on the wall clock of the chosen zone, with its offset, and every wait names the locks
//! behind it; the waiting file's path is written whole, since its end is its name. These functions
//! only build text; each ends with a newline.

use crate::session::session_report::{quoted, shown};

/// What the user is told about when a change may land: the local moment, the zone and the locks.
pub struct Wait<'a> {
    pub local: &'a str,
    pub zone: &'a str,
    pub because: &'a [String],
}

fn held_by(wait: &Wait) -> String {
    wait.because.join("; ")
}

/// The draft of a switch-off that would be made now.
pub fn draft_now(id: &str, command: &str) -> String {
    format!("Draft: the block {} would be switched off now. Nothing was saved.\nTo switch it off, run: {command} --write\n", shown(id))
}

/// The draft of a switch-off that has to wait: it would be parked, not made.
pub fn draft_waits(id: &str, wait: &Wait, waiting_file: &str, command: &str) -> String {
    format!(
        "Draft: the block {} cannot be switched off yet. The change would be parked in {} and may land from {} ({}), held by: {}.\nNothing was saved.\nTo park it, run: {command} --write\n",
        shown(id),
        quoted(waiting_file),
        wait.local,
        wait.zone,
        held_by(wait)
    )
}

/// The result of a switch-off that was made now.
pub fn saved_now(id: &str, file: &str) -> String {
    format!("Saved: the block {} is switched off in {}.\n", shown(id), shown(file))
}

/// The result of a switch-off that was parked.
pub fn parked(id: &str, wait: &Wait, waiting_file: &str, command: &str) -> String {
    format!(
        "Waiting: the block {} is not switched off yet. It may be switched off from {} ({}), held by: {}.\nThe change is parked in {}. When it is due, run: {command}\n",
        shown(id),
        wait.local,
        wait.zone,
        held_by(wait),
        quoted(waiting_file)
    )
}

pub fn already_off(id: &str) -> String {
    format!("The block {} is already off. Nothing was saved.\n", shown(id))
}

/// A block whose change is already parked: when it may land, and what to do once it is due.
pub fn already_waiting(id: &str, local: &str, zone: &str, due_now: bool, settle_command: &str) -> String {
    let next = if due_now { format!(" It is due now: run {settle_command}") } else { String::new() };
    format!("The block {} is already waiting: it may be switched off from {local} ({zone}). Nothing was saved.{next}\n", shown(id))
}

/// The refusal for an id that no block has.
pub fn no_such_block(id: &str, file: &str) -> String {
    format!("There is no block {} in {}.", shown(id), shown(file))
}

pub fn needs_partner_key(id: &str) -> String {
    format!("A lock on the block {} needs a partner's key, and this program cannot give one.", shown(id))
}

pub fn cannot_land(id: &str) -> String {
    format!("The locks on the block {} never leave a moment to change it, so it cannot be switched off by itself.", shown(id))
}
