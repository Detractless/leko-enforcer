//! Reads the settings zip and its waiting file for a delayed change request, so every command
//! refuses a bad file the same way.

use super::waiting_file_disk::{read_waiting_changes, waiting_path};
use crate::session::run_command::{fail, EXIT_REFUSED};
use crate::session::session_report::refusal_line;
use crate::status::settings_file::read_settings_and_bytes;
use leko_core::delayed_changes::waiting_change::WaitingChange;
use leko_core::models::specs::ReadSet;
use std::io::Write;

/// The settings zip bytes and its set, plus where the waiting records live and what they hold.
pub struct RequestFiles {
    pub bytes: Vec<u8>,
    pub set: ReadSet,
    pub waiting_file: String,
    pub changes: Vec<WaitingChange>,
}

/// Reads `file` and the waiting file beside it. A bad file is reported to `err` and its exit code
/// returned, so the caller returns it unchanged.
pub fn read_request_files(file: &str, err: &mut dyn Write) -> Result<RequestFiles, i32> {
    let (bytes, set) = match read_settings_and_bytes(file) {
        Ok(read) => read,
        Err(sentence) => return Err(fail(err, EXIT_REFUSED, &refusal_line(&sentence))),
    };
    let waiting_file = waiting_path(file);
    let changes = match read_waiting_changes(&waiting_file) {
        Ok(found) => found.unwrap_or_default(),
        Err(sentence) => return Err(fail(err, EXIT_REFUSED, &refusal_line(&sentence))),
    };
    Ok(RequestFiles { bytes, set, waiting_file, changes })
}
