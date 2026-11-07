//! The sentences of `leko switch-on`: the draft, the saved result, the answer for a block that is
//! already on and the stop for a block whose switch-off is still parked. The block id and the settings
//! file are shown with `shown`, so an odd or long one cannot flood or steer the terminal; the commands
//! the user is told to run are written whole with `quoted`, since they are meant to be copied. These
//! functions only build text; each ends with a newline (the stop is passed through `refusal_line`).

use crate::session::session_report::{quoted, shown};

/// The draft of a switch-on: what would be made, and the command that does it.
pub fn draft_on(id: &str, command: &str) -> String {
    format!("Draft: the block {} would be switched on now. Nothing was saved.\nTo switch it on, run: {command} --write\n", shown(id))
}

/// The result of a switch-on that was saved.
pub fn saved_on(id: &str, file: &str) -> String {
    format!("Saved: the block {} is switched on in {}.\n", shown(id), shown(file))
}

/// A block that is on already: there is nothing to change.
pub fn already_on(id: &str) -> String {
    format!("The block {} is already on. Nothing was saved.\n", shown(id))
}

/// The stop for a block whose switch-off is parked: it would switch the block off again, so it must be
/// cancelled first. Written without the closing words, which `refusal_line` adds.
pub fn switch_off_waiting(id: &str, file: &str) -> String {
    format!(
        "The block {} has a switch-off waiting, which would undo a switch-on. Cancel it first: leko cancel {} --id {} --write",
        shown(id),
        quoted(file),
        quoted(id)
    )
}
