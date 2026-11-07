//! Runs one `leko status` request in order: the words are read (exit 2 when they are not understood,
//! before anything else is touched), the zone is chosen, the settings file is read (exit 1 for either
//! when there is no answer, with nothing on stdout), then the core decides and finds the next flip at
//! the moment it is given. The parked changes of the blocks that cover the target are added from the
//! waiting file; when that file is bad the answer still goes out (exit 0) and the problem goes to
//! `err`. The answer goes to `out`, every error to `err`. Nothing is saved, and the clock and the
//! computer's zone come in as arguments.

use super::settings_file::read_settings;
use super::status_arguments::{read_status_arguments, StatusTarget};
use super::status_report::{json_line, sentence, Answer};
use super::zone_choice::choose_zone;
use crate::session::run_command::{fail, write_result, EXIT_NOT_UNDERSTOOD, EXIT_REFUSED};
use crate::session::session_arguments::USAGE;
use crate::waiting_changes::status_waiting::read_waiting_for;
use leko_core::decision::block_cover::Target;
use leko_core::decision::decide_target::decide;
use leko_core::status::next_flip::next_flip;
use std::collections::HashSet;
use std::io::Write;

/// Runs the status request in `words` (the words after `status`) at the moment `now_ms`, in the zone
/// `--zone` names or else `computer_zone`, and returns the exit code.
pub fn run_status(words: &[String], now_ms: i64, computer_zone: Option<&str>, out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    let request = match read_status_arguments(words) {
        Ok(request) => request,
        Err(problem) => return fail(err, EXIT_NOT_UNDERSTOOD, &format!("{problem}\n{USAGE}\n")),
    };
    let (zone_name, zone) = match choose_zone(request.zone.as_deref(), computer_zone) {
        Ok(chosen) => chosen,
        Err(sentence) => return fail(err, EXIT_REFUSED, &format!("{sentence}\n")),
    };
    let set = match read_settings(&request.file) {
        Ok(set) => set,
        Err(sentence) => return fail(err, EXIT_REFUSED, &format!("{sentence}\n")),
    };
    let target = match &request.target {
        StatusTarget::App(name) => Target::App(name),
        StatusTarget::Site(host) => Target::Host(host),
    };
    // The list of apps that are never blocked is a later step; until then it is empty.
    let never_blocked = HashSet::new();
    let decision = decide(&set, target, now_ms, &zone, &never_blocked);
    let until = next_flip(&set, target, &never_blocked, now_ms, &zone);
    // A waiting file that cannot be read does not take the answer down: it is said on `err`, and the
    // answer goes out without the list.
    let waiting = match read_waiting_for(&request.file, &set, target, now_ms, &zone) {
        Ok(steps) => Some(steps),
        Err(sentence) => {
            let _ = err.write_all(format!("{sentence}\n").as_bytes());
            None
        }
    };
    let answer = Answer { target: &request.target, at_ms: now_ms, zone_name: &zone_name, zone: &zone, decision: &decision, until, waiting: waiting.as_deref() };
    write_result(out, err, &if request.json { json_line(&answer) } else { sentence(&answer) })
}
