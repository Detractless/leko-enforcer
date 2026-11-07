//! Finds the waiting file beside a settings file, reads it from disk (at most 1 MB) and saves it. A
//! file that is not there means nothing is waiting; every other problem is one plain sentence that
//! names the file safely, and a file that was refused is never replaced. The save goes through the
//! same temporary-file-then-move step as every other save, and makes the file when it is new.

use crate::add_block::file_replace::{save_file, Steps, WAITING_WORDS};
use crate::session::session_report::shown;
use leko_core::delayed_changes::waiting_change::WaitingChange;
use leko_core::delayed_changes::waiting_file::{read_waiting, write_waiting, MAX_CHANGES};
use std::io::{ErrorKind, Read};

const MAX_WAITING_BYTES: u64 = 1_048_576;

/// The waiting file of the settings file `settings`: its name and `.waiting.json`, in the same folder.
pub fn waiting_path(settings: &str) -> String {
    format!("{settings}.waiting.json")
}

fn unreadable(path: &str, problem: &std::io::Error) -> String {
    format!("The waiting file {} could not be read ({problem}). It was left as it was.", shown(path))
}

/// The sentence for a file the core refused: `reason` follows the file's name, or after a colon when
/// it names a record.
fn refused(path: &str, reason: &str) -> String {
    let join = if reason.starts_with("change ") { "was refused: " } else { "" };
    format!("The waiting file {} {join}{reason}. It was left as it was.", shown(path))
}

/// The changes parked in the waiting file at `path`, or `None` when there is no such file.
pub fn read_waiting_changes(path: &str) -> Result<Option<Vec<WaitingChange>>, String> {
    let facts = match std::fs::metadata(path) {
        Ok(facts) => facts,
        Err(problem) if problem.kind() == ErrorKind::NotFound && std::fs::symlink_metadata(path).is_err() => return Ok(None),
        Err(problem) => return Err(unreadable(path, &problem)),
    };
    if !facts.is_file() {
        return Err(format!("The waiting file {} is not a file. It was left as it was.", shown(path)));
    }
    let too_big = format!("The waiting file {} is larger than 1 MB, so it was not read. It was left as it was.", shown(path));
    if facts.len() > MAX_WAITING_BYTES {
        return Err(too_big);
    }
    let file = std::fs::File::open(path).map_err(|problem| unreadable(path, &problem))?;
    let mut bytes = Vec::new();
    file.take(MAX_WAITING_BYTES + 1).read_to_end(&mut bytes).map_err(|problem| unreadable(path, &problem))?;
    if bytes.len() as u64 > MAX_WAITING_BYTES {
        return Err(too_big);
    }
    let text = String::from_utf8(bytes).map_err(|_| refused(path, "is not valid JSON"))?;
    read_waiting(&text).map(Some).map_err(|reason| refused(path, &reason))
}

/// Saves `changes` as the waiting file at `path` (made when it is not there), or gives the sentence
/// saying why it could not be saved. The old file is as it was unless the whole save worked.
pub fn save_waiting(path: &str, changes: &[WaitingChange], steps: &mut dyn Steps) -> Result<(), String> {
    if changes.len() > MAX_CHANGES {
        return Err(format!("The waiting file {} would hold more than {MAX_CHANGES} changes, so nothing was parked.", shown(path)));
    }
    let text = write_waiting(changes).map_err(|reason| format!("The waiting file {} could not be written: {reason}.", shown(path)))?;
    if text.len() as u64 > MAX_WAITING_BYTES {
        return Err(format!("The waiting file {} would be larger than 1 MB, so nothing was parked.", shown(path)));
    }
    save_file(path, text.as_bytes(), steps, WAITING_WORDS, true)
}
