//! The sentences of `leko cancel`: the draft, the saved result and the answer for a block that has
//! nothing waiting. The waiting file's path is written whole with `quoted`, since its end is its
//! name, and the block id with `shown`, so an odd or long one cannot flood or steer the terminal.
//! These functions only build text; each ends with a newline.

use crate::session::session_report::{quoted, shown};

/// The draft of a cancel: what would be taken back, and the command that does it.
pub fn draft(id: &str, command: &str) -> String {
    format!("Draft: the waiting change for the block {} would be cancelled. Nothing was saved.\nTo cancel it, run: {command} --write\n", shown(id))
}

/// The result of a cancel that was saved.
pub fn cancelled(id: &str, waiting_file: &str) -> String {
    format!("Cancelled: the waiting change for the block {} is taken back and removed from {}.\n", shown(id), quoted(waiting_file))
}

/// A block with no parked change (or no waiting file at all): there is nothing to take back.
pub fn nothing_to_cancel(id: &str) -> String {
    format!("Nothing is waiting for the block {}. Nothing was saved.\n", shown(id))
}
