//! Reads the program arguments (the program name already cut off) into one request: the command,
//! its flags, and the typed length or the heard words. Nothing is guessed: a missing, repeated,
//! unknown or odd argument is a one-sentence problem and no request. This file reads no files and
//! no clock; it only looks at the text it is given.

use super::session_report::shown;
use std::ffi::OsString;

pub const DEFAULT_RULE_ID: &str = "session";

pub const USAGE: &str = "Usage: leko session [--json] [--id NAME] LENGTH   (LENGTH is minutes, or a number with m, h or d: 90, 90m, 2h, 7d)\n       leko voice [--json] [--id NAME] WORDS...   (WORDS is what was heard, such as \"start a 25 minute session\")\n       leko status [--json] [--zone ZONE] (--app NAME | --site HOST | --device) FILE.zip   (is it closed now, and until when)\n       leko add-block FILE.zip --name NAME ((--app ID | --site HOST)... | --device lock|logoff|shutdown) (--always | --open-hours HH:MM-HH:MM[,..] | --for LENGTH) [--days mon,tue] [--wall | --wait SECONDS] [--message TEXT] [--write]   (shows the new block; saves it only with --write)\n       leko edit-block FILE.zip --id ID [--zone ZONE] (--add-app ID | --remove-app ID | --add-site HOST | --remove-site HOST)... [--write]\n       leko switch-off FILE.zip --id ID [--zone ZONE] [--write]   (switches a block off now, or parks the change when a lock makes it wait)\n       leko settle FILE.zip [--zone ZONE] [--write]   (lands the parked changes that are due)\n       leko cancel FILE.zip --id ID [--write]   (takes a parked change back; needs no wait)\n       leko switch-on FILE.zip --id ID [--write]   (switches a block back on; needs no wait)\n       leko agent install --executable PATH --settings PATH [--write] | status | uninstall [--write]\n       leko --help";

/// How the length reaches the command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Way {
    /// `leko session LENGTH`.
    Typed,
    /// `leko voice WORDS...`.
    Heard,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    pub way: Way,
    pub rule_id: String,
    pub json: bool,
    /// The typed length, or the heard words joined by one space.
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Parsed {
    Help,
    Make(Request),
    /// `leko status`: the words after the command, read by the status command itself.
    Status(Vec<String>),
    /// `leko add-block`: the words after the command, read by the add-block command itself.
    AddBlock(Vec<String>),
    EditBlock(Vec<String>),
    EditSchedule(Vec<String>),
    /// `leko switch-off`: the words after the command, read by the switch-off command itself.
    SwitchOff(Vec<String>),
    /// `leko settle`: the words after the command, read by the settle command itself.
    Settle(Vec<String>),
    /// `leko cancel`: the words after the command, read by the cancel command itself.
    Cancel(Vec<String>),
    /// `leko switch-on`: the words after the command, read by the switch-on command itself.
    SwitchOn(Vec<String>),
    /// What is wrong with the arguments, in one sentence.
    Bad(String),
}

fn bad(sentence: &str) -> Parsed {
    Parsed::Bad(sentence.to_string())
}

/// The flags and the other words after the command, or the problem found.
fn split_flags<'a>(rest: &[&'a str]) -> Result<(bool, Option<String>, Vec<&'a str>), Parsed> {
    let (mut json, mut rule_id, mut others) = (false, None, Vec::new());
    let mut index = 0;
    while index < rest.len() {
        match rest[index] {
            "--json" if json => return Err(bad("--json was given twice.")),
            "--json" => json = true,
            "--id" if rule_id.is_some() => return Err(bad("--id was given twice.")),
            "--id" => match rest.get(index + 1) {
                Some(name) if !name.starts_with('-') => {
                    rule_id = Some((*name).to_string());
                    index += 1;
                }
                _ => return Err(bad("--id needs a name after it.")),
            },
            flag if flag.starts_with('-') => return Err(Parsed::Bad(format!("{} is not a flag of this command.", shown(flag)))),
            word => others.push(word),
        }
        index += 1;
    }
    Ok((json, rule_id, others))
}

/// The request in `args`, a request for help, or the problem with them.
pub fn read_arguments(args: &[OsString]) -> Parsed {
    let mut texts: Vec<&str> = Vec::with_capacity(args.len());
    for arg in args {
        match arg.to_str() {
            Some(text) => texts.push(text),
            None => return bad("An argument is not valid text."),
        }
    }
    if texts.iter().any(|text| matches!(*text, "--help" | "-h")) {
        return Parsed::Help;
    }
    let Some((&command, rest)) = texts.split_first() else {
        return bad("No command was given.");
    };
    if command == "status" {
        return Parsed::Status(rest.iter().map(|word| (*word).to_string()).collect());
    }
    if command == "add-block" {
        return Parsed::AddBlock(rest.iter().map(|word| (*word).to_string()).collect());
    }
    if command == "edit-schedule" {
        return Parsed::EditSchedule(rest.iter().map(|word| (*word).to_string()).collect());
    }
    if command == "edit-block" {
        return Parsed::EditBlock(rest.iter().map(|word| (*word).to_string()).collect());
    }
    if command == "switch-off" {
        return Parsed::SwitchOff(rest.iter().map(|word| (*word).to_string()).collect());
    }
    if command == "settle" {
        return Parsed::Settle(rest.iter().map(|word| (*word).to_string()).collect());
    }
    if command == "cancel" {
        return Parsed::Cancel(rest.iter().map(|word| (*word).to_string()).collect());
    }
    if command == "switch-on" {
        return Parsed::SwitchOn(rest.iter().map(|word| (*word).to_string()).collect());
    }
    let way = match command {
        "session" => Way::Typed,
        "voice" => Way::Heard,
        other => return Parsed::Bad(format!("{} is not a command.", shown(other))),
    };
    let (json, rule_id, others) = match split_flags(rest) {
        Ok(found) => found,
        Err(problem) => return problem,
    };
    let text = match (way, others.as_slice()) {
        (Way::Typed, [length]) => (*length).to_string(),
        (Way::Typed, []) => return bad("No length was given."),
        (Way::Typed, _) => return bad("Only one length can be given."),
        (Way::Heard, []) => return bad("No words were given."),
        (Way::Heard, words) => words.join(" "),
    };
    Parsed::Make(Request { way, rule_id: rule_id.unwrap_or_else(|| DEFAULT_RULE_ID.to_string()), json, text })
}
