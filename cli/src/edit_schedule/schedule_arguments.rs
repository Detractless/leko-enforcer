//! Parse schedule edit arguments without touching files or operating-system state.
use leko_core::config::strict_fields::parse_minutes;
use leko_core::edit_schedule::rule_edit::WEEKDAYS;

pub const USAGE: &str = "Usage: leko edit-schedule FILE --id BLOCK --rule RULE [--open-hours HH:MM-HH:MM[,..]] [--days mon,tue|all] [--zone ZONE] [--write]";

pub struct Request {
    pub file: String,
    pub id: String,
    pub rule: String,
    pub days: Option<Vec<String>>,
    pub stretches: Option<Vec<(i64, i64)>>,
    pub zone: Option<String>,
    pub write: bool,
}

fn parse_hours(text: &str) -> Result<Vec<(i64, i64)>, String> {
    let mut stretches = Vec::new();
    for part in text.split(',') {
        let (from_str, to_str) = part.split_once('-')
            .ok_or_else(|| format!("Invalid hours stretch {part:?}: expected HH:MM-HH:MM"))?;
        let from = parse_minutes(from_str)
            .ok_or_else(|| format!("Invalid start time {from_str:?} in stretch {part:?}"))?;
        let to = parse_minutes(to_str)
            .ok_or_else(|| format!("Invalid end time {to_str:?} in stretch {part:?}"))?;
        stretches.push((from, to));
    }
    Ok(stretches)
}

fn parse_days(text: &str) -> Result<Vec<String>, String> {
    if text == "all" {
        return Ok(WEEKDAYS.iter().map(|d| d.to_string()).collect());
    }
    let mut days = Vec::new();
    for day in text.split(',') {
        let clean = day.trim().to_lowercase();
        if !WEEKDAYS.contains(&clean.as_str()) {
            return Err(format!("Invalid weekday {day:?}. Use mon, tue, wed, thu, fri, sat, sun, or all."));
        }
        if days.contains(&clean) {
            return Err(format!("Weekday {day:?} is specified more than once."));
        }
        days.push(clean);
    }
    Ok(days)
}

pub fn parse(words: &[String]) -> Result<Request, String> {
    let (mut file, mut id, mut rule, mut days, mut stretches, mut zone, mut write) =
        (None, None, None, None, None, None, false);
    let mut index = 0;
    while index < words.len() {
        let word = &words[index];
        if word == "--write" {
            if write {
                return Err("--write was given twice.".into());
            }
            write = true;
        } else if matches!(word.as_str(), "--id" | "--rule" | "--days" | "--open-hours" | "--zone") {
            index += 1;
            let value = words.get(index).filter(|v| !v.is_empty() && !v.starts_with('-'))
                .ok_or_else(|| format!("{word} needs a value."))?;
            match word.as_str() {
                "--id" => {
                    if id.replace(value.clone()).is_some() {
                        return Err("--id was given twice.".into());
                    }
                }
                "--rule" => {
                    if rule.replace(value.clone()).is_some() {
                        return Err("--rule was given twice.".into());
                    }
                }
                "--zone" => {
                    if zone.replace(value.clone()).is_some() {
                        return Err("--zone was given twice.".into());
                    }
                }
                "--days" => {
                    if days.is_some() {
                        return Err("--days was given twice.".into());
                    }
                    days = Some(parse_days(value)?);
                }
                "--open-hours" => {
                    if stretches.is_some() {
                        return Err("--open-hours was given twice.".into());
                    }
                    stretches = Some(parse_hours(value)?);
                }
                _ => unreachable!(),
            }
        } else if word.starts_with('-') {
            return Err(format!("Unknown flag {word:?}."));
        } else if file.replace(word.clone()).is_some() {
            return Err("Only one settings file can be given.".into());
        }
        index += 1;
    }
    let file = file.ok_or("A settings file is required.")?;
    let id = id.ok_or("--id is required.")?;
    let rule = rule.ok_or("--rule is required.")?;
    if days.is_none() && stretches.is_none() {
        return Err("Give at least one of --open-hours or --days to edit.".into());
    }
    Ok(Request { file, id, rule, days, stretches, zone, write })
}
