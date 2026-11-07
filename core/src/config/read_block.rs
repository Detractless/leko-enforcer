//! Reads a block file in the phone's order of reads and, as the phone does, checks the rules that
//! tie its fields together. Ported from Enforcer's `lib/read-block.js` and `lib/read-rules.js`.

use super::read_door::{read_door, read_earn, read_pass};
use super::read_key_lock::read_lock;
use super::strict_fields::{Read, StrictFields, MAX_INT, MINUTES_PER_DAY};
use crate::models::specs::{AppCover, BlockSpec, Door, ReadMode, Rule};
use crate::sessions::session_rule::read_session;
use crate::sites::site_name::read_sites;
use serde_json::Value;
use std::collections::HashSet;

pub const MAX_ID: i64 = 64;
pub const ID_PROBLEM: &str = "must hold only letters, digits, - and _, up to 64 characters";
const MAX_PACKAGE: i64 = 255;
const MAX_MESSAGE: i64 = 300;
const MAX_NAME: i64 = 60;
const MAX_RULES: usize = 16;
const MAX_STRETCHES: i64 = 12;
const MAX_AMOUNT: i64 = 1_000_000;
const KINDS: [&str; 5] = ["always", "everyOpen", "outsideHours", "minutes", "opens"];
const CLOSES_BY_DOOR: [&str; 3] = ["always", "everyOpen", "outsideHours"];

/// What a reader needs to know besides the file: the id its name gives it, and the packages
/// Enforcer never blocks.
pub struct ReadContext<'a> {
    pub expected_id: Option<&'a str>,
    pub always_allowed: &'a HashSet<String>,
}

/// The id of an item: ASCII letters, digits, - and _, up to 64 of them.
pub fn is_item_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= MAX_ID as usize && id.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
}

/// A package name: dot-separated parts of letters, digits, - and _.
pub fn is_package(name: &str) -> bool {
    name.split('.').all(|part| !part.is_empty() && part.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_'))
}

/// The id of an item: well formed, and the same as the file name.
pub fn read_item_id(fields: &mut StrictFields, context: &ReadContext) -> Read<String> {
    let id = fields.text_len("id", 1, MAX_ID)?;
    if !is_item_id(&id) {
        return fields.fail("id", ID_PROBLEM);
    }
    match context.expected_id {
        Some(expected) if expected != id => fields.fail("id", &format!("must equal the file name \"{expected}\"")),
        _ => Ok(id),
    }
}

fn read_packages(fields: &mut StrictFields, name: &str, minimum: i64) -> Read<Vec<String>> {
    let packages = fields.texts(name, minimum, MAX_INT, 1, MAX_PACKAGE, true)?.unwrap_or_default();
    for (index, text) in packages.iter().enumerate() {
        if !is_package(text) {
            return fields.fail(&format!("{name}[{index}]"), "must be a package name");
        }
    }
    Ok(packages)
}

/// The covers object. Returns the apps it covers and its sites, which only the extended mode reads:
/// there `sites` may stand alone or beside `apps` or `allAppsExcept`; in the phone's mode it is an
/// unknown field.
fn read_covers(parent: &mut StrictFields, context: &ReadContext) -> Read<(AppCover, Vec<String>)> {
    let mut fields = parent.child_required("covers")?;
    let has_sites = fields.mode() == ReadMode::Extended && fields.has("sites");
    let has_apps = fields.has("apps");
    let has_except = fields.has("allAppsExcept");
    if has_apps == has_except && !(has_sites && !has_apps) {
        return fields.fail("", "must hold either \"apps\" or \"allAppsExcept\"");
    }
    let apps = if has_apps {
        let packages = read_packages(&mut fields, "apps", 1)?;
        if let Some(never) = packages.iter().find(|name| context.always_allowed.contains(*name)) {
            return fields.fail("apps", &format!("cannot name {never}, which Enforcer never blocks"));
        }
        AppCover::Named(packages)
    } else if has_except {
        AppCover::AllExcept(read_packages(&mut fields, "allAppsExcept", 0)?)
    } else {
        AppCover::Nothing
    };
    let sites = if has_sites { read_sites(&mut fields)? } else { Vec::new() };
    fields.finish()?;
    Ok((apps, sites))
}

fn read_per(fields: &mut StrictFields, allowed: &[&str]) -> Read<()> {
    let text = fields.text("per")?;
    if !allowed.contains(&text.as_str()) {
        return fields.fail("per", &format!("must be {}", allowed.join(", ")));
    }
    Ok(())
}

fn read_scope(fields: &mut StrictFields) -> Read<()> {
    let text = fields.text("scope")?;
    if text != "block" && text != "app" {
        return fields.fail("scope", "must be block or app");
    }
    Ok(())
}

/// True when two stretches share a minute of the 24-hour circle.
fn overlaps(stretches: &[(i64, i64)]) -> bool {
    let mut covered = vec![false; MINUTES_PER_DAY as usize];
    for &(from, to) in stretches {
        let mut minute = from;
        while minute != to % MINUTES_PER_DAY {
            if covered[minute as usize] {
                return true;
            }
            covered[minute as usize] = true;
            minute = (minute + 1) % MINUTES_PER_DAY;
        }
    }
    false
}

fn read_stretches(fields: &mut StrictFields) -> Read<Vec<(i64, i64)>> {
    let entries = fields.objects("allowed", 0, MAX_STRETCHES, true)?.unwrap_or_default();
    let mut stretches = Vec::new();
    for mut entry in entries {
        let from = entry.clock("from", false)?;
        let to = entry.clock("to", true)?;
        if from == to {
            return entry.fail("to", "must differ from \"from\"");
        }
        entry.finish()?;
        stretches.push((from, to));
    }
    if overlaps(&stretches) {
        return fields.fail("allowed", "has stretches that overlap");
    }
    Ok(stretches)
}

fn read_rule(fields: &mut StrictFields) -> Read<Rule> {
    let id = fields.text_len("id", 1, MAX_ID + "~was".len() as i64)?;
    if !is_item_id(&id) {
        return fields.fail("id", ID_PROBLEM);
    }
    let days = fields.days("days")?;
    let kind = fields.text("kind")?;
    let (mut allowed, mut ends_at) = (Vec::new(), None);
    match kind.as_str() {
        "outsideHours" => allowed = read_stretches(fields)?,
        "minutes" => {
            fields.whole("amount", 0, MAX_AMOUNT)?;
            read_per(fields, &["hour", "day", "week"])?;
            read_scope(fields)?;
        }
        "opens" => {
            fields.whole("count", 1, MAX_AMOUNT)?;
            read_per(fields, &["day", "week"])?;
            read_scope(fields)?;
        }
        "session" if fields.mode() == ReadMode::Extended => {
            ends_at = Some(read_session(fields)?);
        }
        other if !KINDS.contains(&other) => {
            return fields.fail("kind", &format!("must be always, everyOpen, outsideHours, minutes or opens, not \"{kind}\""));
        }
        _ => {}
    }
    fields.finish()?;
    Ok(Rule { id, kind, days, allowed, ends_at })
}

/// The rules of a block; the first unreadable rule or the first repeated id refuses the file.
fn read_rules(parent: &mut StrictFields) -> Read<Vec<Rule>> {
    let entries = parent.list("rules", true)?.map(Vec::as_slice).unwrap_or_default();
    if entries.len() > MAX_RULES {
        return parent.fail("rules", &format!("must hold 0 to {MAX_RULES} entries"));
    }
    let base = parent.join("rules");
    let mut rules: Vec<Rule> = Vec::new();
    for (index, element) in entries.iter().enumerate() {
        let Value::Object(map) = element else {
            return parent.fail(&format!("rules[{index}]"), "must be an object");
        };
        rules.push(read_rule(&mut StrictFields::new(map, &format!("{base}[{index}]")).with_mode(parent.mode()))?);
    }
    // The first id, by where it first appears, that appears again.
    for rule in &rules {
        if rules.iter().filter(|other| other.id == rule.id).count() > 1 {
            return parent.fail("rules", &format!("has two rules with the id \"{}\"", rule.id));
        }
    }
    if rules.iter().filter(|rule| rule.kind == "session").count() > 1 {
        return parent.fail("rules", "has more than one session rule");
    }
    Ok(rules)
}

fn check_pass(fields: &StrictFields, block: &BlockSpec) -> Read<()> {
    let closes = block.rules.iter().any(|rule| CLOSES_BY_DOOR.contains(&rule.kind.as_str()));
    let walking = matches!(block.door, Door::Walking { .. });
    // A walking door gives no pass: the pace keeps the app open.
    if walking && block.pass.is_some() {
        return fields.fail("pass", "is not used: a walking door gives no pass");
    }
    let needs_pass = !matches!(block.door, Door::Wall) && !walking && closes;
    if needs_pass && block.pass.is_none() {
        return fields.fail("pass", "is required when the door is not a wall and the block has an always, everyOpen or outsideHours rule");
    }
    if !needs_pass && block.pass.is_some() {
        return fields.fail("pass", "is not used: it needs a door that is not a wall and an always, everyOpen or outsideHours rule");
    }
    if block.pass.as_ref().is_some_and(|pass| pass.rising_cost == "double") {
        let doubles = matches!(block.door, Door::Wait { seconds } if seconds >= 1) || matches!(block.door, Door::Practice { .. });
        if !doubles {
            return fields.fail("pass.risingCost", "double needs a wait door of at least 1 second or a practice door");
        }
    }
    Ok(())
}

fn check_earn(fields: &StrictFields, block: &BlockSpec) -> Read<()> {
    if block.earn.is_none() {
        return Ok(());
    }
    if !block.rules.iter().any(|rule| rule.kind == "minutes") {
        return fields.fail("earn", "needs a minutes rule");
    }
    if block.rules.iter().any(|rule| rule.kind == "always" || rule.kind == "everyOpen") {
        return fields.fail("earn", "cannot be used beside an always or everyOpen rule");
    }
    Ok(())
}

/// Reads a block file. Returns what the cross-file checks need.
pub fn read_block(fields: &mut StrictFields, context: &ReadContext) -> Read<BlockSpec> {
    let id = read_item_id(fields, context)?;
    fields.text_len("name", 1, MAX_NAME)?;
    let on = fields.flag("on")?.unwrap_or(true);
    fields.text_opt("message", 0, MAX_MESSAGE)?;
    let (apps, sites) = read_covers(fields, context)?;
    let rules = read_rules(fields)?;
    let door = read_door(fields)?;
    let pass = read_pass(fields)?;
    let earn = read_earn(fields)?;
    let lock = read_lock(fields)?;
    fields.finish()?;
    let block = BlockSpec { id, on, apps, sites, rules, door, pass, earn, lock };
    check_pass(fields, &block)?;
    check_earn(fields, &block)?;
    Ok(block)
}
