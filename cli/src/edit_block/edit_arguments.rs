//! Parse explicit coverage-edit arguments without touching the filesystem.
use leko_core::delayed_changes::waiting_change::{CoverageAction, CoverageOperation, MAX_EDIT_OPERATIONS};

pub struct Request {
    pub file: String,
    pub id: String,
    pub zone: Option<String>,
    pub write: bool,
    pub operations: Vec<CoverageOperation>,
}

pub fn parse(words: &[String]) -> Result<Request, String> {
    let (mut file, mut id, mut zone, mut write) = (None, None, None, false);
    let mut operations: Vec<CoverageOperation> = Vec::new();
    let mut index = 0;
    while index < words.len() {
        let word = &words[index];
        if word == "--write" {
            if write { return Err("--write was given twice.".into()); }
            write = true;
        } else if matches!(word.as_str(), "--id" | "--zone" | "--add-app" | "--remove-app" | "--add-site" | "--remove-site") {
            index += 1;
            let value = words.get(index).filter(|value| !value.is_empty() && !value.starts_with('-')).ok_or_else(|| format!("{word} needs a value."))?;
            match word.as_str() {
                "--id" => if id.replace(value.clone()).is_some() { return Err("--id was given twice.".into()); },
                "--zone" => if zone.replace(value.clone()).is_some() { return Err("--zone was given twice.".into()); },
                _ => {
                    let action = match word.as_str() {
                        "--add-app" => CoverageAction::AddApp, "--remove-app" => CoverageAction::RemoveApp,
                        "--add-site" => CoverageAction::AddSite, _ => CoverageAction::RemoveSite,
                    };
                    if operations.iter().any(|op| op.action.is_app() == action.is_app() && op.target == *value) {
                        return Err(format!("Target {value:?} was given more than once."));
                    }
                    operations.push(CoverageOperation { action, target: value.clone() });
                }
            }
        } else if word.starts_with('-') {
            return Err(format!("Unknown flag {word:?}."));
        } else if file.replace(word.clone()).is_some() {
            return Err("Only one settings file can be given.".into());
        }
        index += 1;
    }
    if operations.is_empty() || operations.len() > MAX_EDIT_OPERATIONS { return Err(format!("Give 1 to {MAX_EDIT_OPERATIONS} coverage operations.")); }
    Ok(Request { file: file.ok_or("A settings file is required.")?, id: id.ok_or("--id is required.")?, zone, write, operations })
}
