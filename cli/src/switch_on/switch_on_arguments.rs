//! Reads the words after `leko switch-on` into one request: the settings file, the block to switch on
//! and the write switch. They are the same three words as `leko cancel` takes, so the reader is shared;
//! only the sentence for a missing `--id` is its own. There is no `--zone` and no `--json` (switching on
//! needs no moment and shows none). This file opens no file and reads no clock.

use crate::waiting_changes::cancel_arguments::{read_file_id_write, FileIdWrite};

/// The request in `words` (the program name and the command word already cut off), or the problem
/// with them in one sentence.
pub fn read_switch_on_arguments(words: &[String]) -> Result<FileIdWrite, String> {
    read_file_id_write(words, "Say which block to switch on: --id ID.")
}
