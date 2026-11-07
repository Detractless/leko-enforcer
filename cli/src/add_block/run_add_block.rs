//! Runs one `leko add-block` request in order: the words are read (exit 2 when they are not
//! understood, before any file is touched), the settings file is read, the core drafts the block and
//! adds it to the zip's bytes (exit 1 for any refusal, with nothing on stdout), then the draft is
//! shown or, with `--write`, the new zip is saved over the old one in one step. The result goes to
//! `out`, every error to `err`. The moment comes in as an argument; the clock is never read here.

use super::add_block_arguments::read_add_block_arguments;
use super::add_block_report::{draft_text, saved_text};
use super::file_replace::{replace_file, RealSteps};
use crate::session::run_command::{fail, write_result, EXIT_NOT_UNDERSTOOD, EXIT_REFUSED};
use crate::session::session_arguments::USAGE;
use crate::session::session_report::{quoted, refusal_line};
use crate::status::settings_file::read_settings_bytes;
use leko_core::blocks::add_block::add_block;
use leko_core::blocks::draft_block::draft_block;
use std::io::Write;

/// Runs the add-block request in `words` (the words after `add-block`) at the moment `now_ms`, and
/// returns the exit code.
pub fn run_add_block(words: &[String], now_ms: i64, out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    let request = match read_add_block_arguments(words) {
        Ok(request) => request,
        Err(problem) => return fail(err, EXIT_NOT_UNDERSTOOD, &format!("{problem}\n{USAGE}\n")),
    };
    let bytes = match read_settings_bytes(&request.file) {
        Ok(bytes) => bytes,
        Err(sentence) => return fail(err, EXIT_REFUSED, &refusal_line(&sentence)),
    };
    let draft = match draft_block(&request.block, now_ms) {
        Ok(draft) => draft,
        Err(sentence) => return fail(err, EXIT_REFUSED, &refusal_line(&sentence)),
    };
    let added = match add_block(&bytes, &draft) {
        Ok(added) => added,
        Err(problem) => return fail(err, EXIT_REFUSED, &refusal_line(&problem.sentence())),
    };
    let warning = added.phone_warning.as_deref();
    if !request.write {
        let command = std::iter::once("leko add-block".to_string()).chain(words.iter().map(|word| quoted(word))).collect::<Vec<_>>().join(" ");
        return write_result(out, err, &draft_text(&request.block, &draft, &request.file, warning, &command));
    }
    match replace_file(&request.file, &added.bytes, &mut RealSteps) {
        Ok(()) => write_result(out, err, &saved_text(&draft, &request.file, warning)),
        Err(sentence) => fail(err, EXIT_REFUSED, &format!("{sentence}\n")),
    }
}
