//! Runs one command line: reads the request, makes the session (or answers the status question, or
//! drafts a block, switches one off, lands the parked changes or takes one back) at the moment it is given, writes to the two streams and returns the exit code.
//! Results go to `out`, every error to `err`. Exit 0 means a session was made, a block was shown or
//! saved, or help was printed; 1 a request that was understood but refused; 2 arguments or words that
//! were not understood. The clock is never read here: the moment comes in as an argument.

use super::session_arguments::{read_arguments, Parsed, Request, Way, USAGE};
use super::session_length::read_length;
use super::session_report::{json_line, refusal, sentence, shown};
use super::voice_phrase::{read_phrase, EXAMPLES};
use crate::add_block::run_add_block::run_add_block;
use crate::delayed_changes::run_settle::run_settle;
use crate::delayed_changes::run_switch_off::run_switch_off;
use crate::edit_block::run_edit_block::run_edit_block;
use crate::edit_schedule::run_edit_schedule::run_edit_schedule;
use crate::status::run_status::run_status;
use crate::switch_on::run_switch_on::run_switch_on;
use crate::waiting_changes::run_cancel::run_cancel;
use leko_core::sessions::timed_session::make_timed_session;
use std::ffi::OsString;
use std::io::Write;

pub const EXIT_MADE: i32 = 0;
pub const EXIT_REFUSED: i32 = 1;
pub const EXIT_NOT_UNDERSTOOD: i32 = 2;

/// Writes `text` to `out`; when `out` cannot be written (a closed pipe) says so on `err` and gives
/// the refused code instead of failing loudly.
pub fn write_result(out: &mut dyn Write, err: &mut dyn Write, text: &str) -> i32 {
    if out.write_all(text.as_bytes()).and_then(|()| out.flush()).is_err() {
        let _ = err.write_all(b"The result could not be written to the output.\n");
        return EXIT_REFUSED;
    }
    EXIT_MADE
}

/// Writes `text` to `err` (nothing to do when that fails too) and gives `code`.
pub fn fail(err: &mut dyn Write, code: i32, text: &str) -> i32 {
    let _ = err.write_all(text.as_bytes());
    code
}

/// The minutes the request asks for, or the sentence for stderr when they cannot be read.
fn minutes_of_request(request: &Request) -> Result<i64, String> {
    match request.way {
        Way::Typed => read_length(&request.text).ok_or_else(|| {
            format!("{} is not a length: write whole minutes, or a whole number with m, h or d (90, 90m, 2h, 7d).\n{USAGE}\n", shown(&request.text))
        }),
        Way::Heard => {
            read_phrase(&request.text).map_err(|reason| format!("Nothing was made: {reason}. Heard: {}. Say, for example, {EXAMPLES}.\n", shown(&request.text)))
        }
    }
}

fn make(request: &Request, now_ms: i64, out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    let minutes = match minutes_of_request(request) {
        Ok(minutes) => minutes,
        Err(sentence) => return fail(err, EXIT_NOT_UNDERSTOOD, &sentence),
    };
    match make_timed_session(now_ms, minutes, &request.rule_id) {
        Err(problem) => fail(err, EXIT_REFUSED, &refusal(&problem)),
        Ok(session) => write_result(out, err, &if request.json { json_line(&session) } else { sentence(minutes, &session) }),
    }
}

/// Runs the command line `args` (without the program name) at the moment `now_ms`, in milliseconds
/// since the epoch, and returns the exit code. `zone_name` is the computer's time zone, when the
/// program found one; `leko status`, `leko switch-off` and `leko settle` use it.
pub fn run(args: &[OsString], now_ms: i64, zone_name: Option<&str>, out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    match read_arguments(args) {
        Parsed::Help => write_result(out, err, &format!("{USAGE}\n")),
        Parsed::Bad(problem) => fail(err, EXIT_NOT_UNDERSTOOD, &format!("{problem}\n{USAGE}\n")),
        Parsed::Make(request) => make(&request, now_ms, out, err),
        Parsed::Status(words) => run_status(&words, now_ms, zone_name, out, err),
        Parsed::AddBlock(words) => run_add_block(&words, now_ms, out, err),
        Parsed::EditBlock(words) => run_edit_block(&words, now_ms, zone_name, out, err),
        Parsed::EditSchedule(words) => run_edit_schedule(&words, now_ms, zone_name, out, err),
        Parsed::SwitchOff(words) => run_switch_off(&words, now_ms, zone_name, out, err),
        Parsed::Settle(words) => run_settle(&words, now_ms, zone_name, out, err),
        Parsed::Cancel(words) => run_cancel(&words, out, err),
        Parsed::SwitchOn(words) => run_switch_on(&words, out, err),
    }
}
