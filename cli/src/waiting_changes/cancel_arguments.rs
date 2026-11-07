//! Reads the words after `leko cancel` into one request: the settings file the waiting file sits
//! beside, the block whose parked change is taken back, and the write switch. Nothing is guessed: a
//! missing, repeated, unknown or odd word is a one-sentence problem and no request. There is no
//! `--zone` (a cancel shows no moment) and no `--json`. The reader is shared with `leko switch-on`,
//! which takes the same three words; only the sentence for a missing `--id` differs. This file opens
//! no file and reads no clock.

use crate::delayed_changes::delay_arguments::once;
use crate::session::session_report::shown;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileIdWrite {
    /// The settings zip, as typed; for a cancel only its name is used, to find the waiting file beside it.
    pub file: String,
    /// The block the command is about.
    pub id: String,
    /// True when the change is to be saved, false when only the draft is shown.
    pub write: bool,
}

/// The request of `leko cancel` in `words` (the program name and the command word already cut off),
/// or the problem with them in one sentence.
pub fn read_cancel_arguments(words: &[String]) -> Result<FileIdWrite, String> {
    read_file_id_write(words, "Say which block's waiting change to cancel: --id ID.")
}

/// A file, `--id ID` and `--write` in `words`, or the problem in one sentence; `missing_id` is the
/// sentence for a command with no `--id`.
pub fn read_file_id_write(words: &[String], missing_id: &str) -> Result<FileIdWrite, String> {
    let (mut id, mut write, mut files) = (None, false, Vec::new());
    let texts: Vec<&str> = words.iter().map(String::as_str).collect();
    let mut index = 0;
    while index < texts.len() {
        match texts[index] {
            "--id" => once(&mut id, "--id", texts.get(index + 1), "a block id")?,
            "--write" if write => return Err("--write was given twice.".to_string()),
            "--write" => write = true,
            flag if flag.starts_with('-') => return Err(format!("{} is not a flag of this command.", shown(flag))),
            file => files.push(file),
        }
        index += if texts[index] == "--id" { 2 } else { 1 };
    }
    let file = match files.as_slice() {
        [file] => (*file).to_string(),
        [] => return Err("No settings file was given.".to_string()),
        _ => return Err("Only one settings file can be given.".to_string()),
    };
    let Some(id) = id else { return Err(missing_id.to_string()) };
    Ok(FileIdWrite { file, id, write })
}
