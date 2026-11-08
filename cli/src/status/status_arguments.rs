//! Reads the words after `leko status` into one request: the settings file, one app, one site or the
//! whole computer, an optional zone name and the JSON switch. Nothing is guessed: a missing, repeated,
//! unknown or empty argument, more than one of `--app`, `--site` and `--device`, and two files are
//! each a one-sentence problem and no request. This file opens no file and reads no clock; it only
//! looks at the words it is given.

use crate::session::session_report::shown;

/// What the answer is about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StatusTarget {
    App(String),
    Site(String),
    /// The whole computer: the strongest action of the device blocks that are closed.
    Device,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusRequest {
    /// The settings zip, as typed.
    pub file: String,
    pub target: StatusTarget,
    /// The zone named with `--zone`, which may be empty or unknown (the zone choice judges it).
    pub zone: Option<String>,
    pub json: bool,
}

/// The value of the flag at `words[index]`: the next word, which must not look like another flag.
fn value_after<'a>(words: &[&'a str], index: usize, flag: &str, what: &str) -> Result<&'a str, String> {
    match words.get(index + 1) {
        Some(value) if !value.starts_with('-') => Ok(value),
        _ => Err(format!("{flag} needs {what} after it.")),
    }
}

fn once(slot: &mut Option<String>, flag: &str, value: &str) -> Result<(), String> {
    if slot.is_some() {
        return Err(format!("{flag} was given twice."));
    }
    *slot = Some(value.to_string());
    Ok(())
}

/// The request in `words` (the program name and the word `status` already cut off), or the problem
/// with them in one sentence.
pub fn read_status_arguments(words: &[String]) -> Result<StatusRequest, String> {
    let words: Vec<&str> = words.iter().map(String::as_str).collect();
    let (mut app, mut site, mut zone) = (None, None, None);
    let (mut json, mut files, mut device) = (false, Vec::new(), false);
    let mut index = 0;
    while index < words.len() {
        match words[index] {
            "--json" if json => return Err("--json was given twice.".to_string()),
            "--json" => json = true,
            "--app" => {
                once(&mut app, "--app", value_after(&words, index, "--app", "an app name")?)?;
                index += 1;
            }
            "--site" => {
                once(&mut site, "--site", value_after(&words, index, "--site", "a website host")?)?;
                index += 1;
            }
            "--device" if device => return Err("--device was given twice.".to_string()),
            "--device" => device = true,
            "--zone" => {
                once(&mut zone, "--zone", value_after(&words, index, "--zone", "a time zone name")?)?;
                index += 1;
            }
            flag if flag.starts_with('-') => return Err(format!("{} is not a flag of this command.", shown(flag))),
            word => files.push(word),
        }
        index += 1;
    }
    let target = match (app, site, device) {
        (Some(_), Some(_), _) => return Err("Give either --app or --site, not both.".to_string()),
        (Some(_), None, true) => return Err("Give either --app or --device, not both.".to_string()),
        (None, Some(_), true) => return Err("Give either --site or --device, not both.".to_string()),
        (None, None, false) => return Err("Say what to look at: --app NAME, --site HOST or --device.".to_string()),
        (Some(name), None, false) if name.is_empty() => return Err("--app needs an app name that is not empty.".to_string()),
        (None, Some(host), false) if host.is_empty() => return Err("--site needs a website host that is not empty.".to_string()),
        (Some(name), None, false) => StatusTarget::App(name),
        (None, Some(host), false) => StatusTarget::Site(host),
        (None, None, true) => StatusTarget::Device,
    };
    match files.as_slice() {
        [file] => Ok(StatusRequest { file: (*file).to_string(), target, zone, json }),
        [] => Err("No settings file was given.".to_string()),
        _ => Err("Only one settings file can be given.".to_string()),
    }
}
