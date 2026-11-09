//! Load the request, zone, settings bytes and parked-change queue an edit command needs before it
//! can judge its change.
use crate::delayed_changes::delay_arguments::pick_zone;
use crate::delayed_changes::waiting_file_disk::{read_waiting_changes, waiting_path};
use crate::session::run_command::{fail, EXIT_NOT_UNDERSTOOD, EXIT_REFUSED};
use crate::session::session_report::refusal_line;
use crate::status::settings_file::read_settings_and_bytes;
use leko_core::config_time::zone_time::ZoneTime;
use leko_core::delayed_changes::waiting_change::WaitingChange;
use leko_core::io::zip_check::{check_zip_in_mode, ZipVerdict};
use leko_core::models::specs::{ReadMode, ReadSet};
use std::collections::HashSet;
use std::io::Write;

pub struct EditPrologue<R> {
    pub request: R,
    pub zone_name: String,
    pub zone: ZoneTime,
    pub bytes: Vec<u8>,
    pub set: ReadSet,
    pub waiting_file: String,
    pub waiting: Vec<WaitingChange>,
}

pub trait EditPrologueRequest {
    fn file(&self) -> &str;
    fn zone(&self) -> Option<&str>;
}

pub fn phone_compatibility_warning(bytes: &[u8]) -> Option<String> {
    match check_zip_in_mode(bytes, &HashSet::new(), ReadMode::Phone) {
        ZipVerdict::Read(_) => None,
        ZipVerdict::Refused(reason) => Some(format!(
            "Phone Enforcer 0.3 cannot read these settings: {}.",
            reason.sentence
        )),
    }
}

pub fn read_edit_prologue<R: EditPrologueRequest>(
    words: &[String],
    usage: &str,
    parse: fn(&[String]) -> Result<R, String>,
    computer_zone: Option<&str>,
    err: &mut dyn Write,
) -> Result<EditPrologue<R>, i32> {
    let request = match parse(words) {
        Ok(request) => request,
        Err(problem) => return Err(fail(err, EXIT_NOT_UNDERSTOOD, &format!("{problem}\n{usage}\n"))),
    };
    let (zone_name, zone) = match pick_zone(request.zone(), computer_zone) {
        Ok(zone) => zone,
        Err((code, text)) => return Err(fail(err, code, &text)),
    };
    let (bytes, set) = match read_settings_and_bytes(request.file()) {
        Ok(value) => value,
        Err(problem) => return Err(fail(err, EXIT_REFUSED, &refusal_line(&problem))),
    };
    let waiting_file = waiting_path(request.file());
    let waiting = match read_waiting_changes(&waiting_file) {
        Ok(value) => value.unwrap_or_default(),
        Err(problem) => return Err(fail(err, EXIT_REFUSED, &refusal_line(&problem))),
    };
    Ok(EditPrologue { request, zone_name, zone, bytes, set, waiting_file, waiting })
}
