//! Reads the words after `leko switch-off` and `leko settle` into one request: the settings file,
//! the block id (switch-off only), the zone name and the write switch, and chooses the zone the
//! moments are shown and judged in. Nothing is guessed: a missing, repeated, unknown or odd word is
//! a one-sentence problem and no request. This file opens no file and reads no clock; the computer's
//! zone comes in as an argument.

use crate::session::run_command::{EXIT_NOT_UNDERSTOOD, EXIT_REFUSED};
use crate::session::session_arguments::USAGE;
use crate::session::session_report::{refusal_line, shown};
use crate::status::zone_choice::choose_zone;
use leko_core::config_time::zone_time::ZoneTime;

/// Which of the two commands the words belong to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    SwitchOff,
    Settle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DelayRequest {
    /// The settings zip, as typed.
    pub file: String,
    /// The block to switch off; always set for `switch-off`, never for `settle`.
    pub id: Option<String>,
    /// The zone named with `--zone`, if any.
    pub zone: Option<String>,
    /// True when the change is to be saved, false when only the draft is shown.
    pub write: bool,
}

pub fn once(slot: &mut Option<String>, flag: &str, value: Option<&&str>, what: &str) -> Result<(), String> {
    let Some(value) = value.filter(|value| !value.is_empty() && !value.starts_with('-')) else { return Err(format!("{flag} needs {what} after it.")) };
    if slot.is_some() {
        return Err(format!("{flag} was given twice."));
    }
    *slot = Some((*value).to_string());
    Ok(())
}

/// The request in `words` (the program name and the command word already cut off), or the problem
/// with them in one sentence.
pub fn read_delay_arguments(words: &[String], command: Command) -> Result<DelayRequest, String> {
    let (mut id, mut zone, mut write, mut files) = (None, None, false, Vec::new());
    let texts: Vec<&str> = words.iter().map(String::as_str).collect();
    let mut index = 0;
    while index < texts.len() {
        match texts[index] {
            "--id" if command == Command::Settle => return Err("--id is only for switch-off: settle lands every change that is due.".to_string()),
            "--id" => once(&mut id, "--id", texts.get(index + 1), "a block id")?,
            "--zone" => once(&mut zone, "--zone", texts.get(index + 1), "a zone name")?,
            "--write" if write => return Err("--write was given twice.".to_string()),
            "--write" => write = true,
            flag if flag.starts_with('-') => return Err(format!("{} is not a flag of this command.", shown(flag))),
            file => files.push(file),
        }
        index += if matches!(texts[index], "--id" | "--zone") { 2 } else { 1 };
    }
    let file = match files.as_slice() {
        [file] => (*file).to_string(),
        [] => return Err("No settings file was given.".to_string()),
        _ => return Err("Only one settings file can be given.".to_string()),
    };
    if command == Command::SwitchOff && id.is_none() {
        return Err("Say which block to switch off: --id ID.".to_string());
    }
    Ok(DelayRequest { file, id, zone, write })
}

/// The zone name and the zone to judge in: the one `--zone` names, else the computer's. A zone the
/// user named and this computer does not know is a wrong argument (exit 2, with the usage); a
/// computer zone that is missing or unusable is a refusal (exit 1), because UTC is never assumed.
/// The error is the exit code and the text for stderr.
pub fn pick_zone(flag: Option<&str>, computer: Option<&str>) -> Result<(String, ZoneTime), (i32, String)> {
    choose_zone(flag, computer).map_err(|sentence| match flag {
        Some(_) => (EXIT_NOT_UNDERSTOOD, format!("{sentence}\n{USAGE}\n")),
        None => (EXIT_REFUSED, refusal_line(&sentence)),
    })
}
