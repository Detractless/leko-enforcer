use std::io::Write;

use leko_core::models::{Pass, Penalty, Schedule, Seal, Target};
use leko_core::TechnologyManagementRule;

use crate::arguments::CliArgs;
use crate::clock::CliClock;
use crate::commands::{emit_json, emit_text, load, refused, save, CommandResult};
use crate::services::{output_human, output_json};

fn slug(name: &str) -> String {
    name.to_lowercase()
        .chars()
        .map(|value| if value.is_alphanumeric() { value } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

pub fn run(args: &CliArgs, out: &mut dyn Write) -> CommandResult {
    let name = args
        .flags
        .get("name")
        .ok_or_else(|| refused("The flag --name needs a value.", 2))?;
    let target_text = args
        .flags
        .get("target")
        .ok_or_else(|| refused("The flag --target needs a value.", 2))?;
    let target: Target = serde_json::from_str(target_text)
        .map_err(|error| refused(format!("Invalid target: {}", error), 1))?;
    let schedule: Schedule = match args.flags.get("schedule") {
        Some(value) => serde_json::from_str(value)
            .map_err(|error| refused(format!("Invalid schedule: {}", error), 1))?,
        None => Schedule::default(),
    };
    let penalty: Option<Penalty> = match args.flags.get("penalty") {
        Some(value) => serde_json::from_str(value)
            .map(Some)
            .map_err(|error| refused(format!("Invalid penalty: {}", error), 1))?,
        None => None,
    };
    let pass: Pass = match args.flags.get("pass") {
        Some(value) => serde_json::from_str(value)
            .map_err(|error| refused(format!("Invalid pass: {}", error), 1))?,
        None => Pass::default(),
    };
    let seal: Seal = match args.flags.get("seal") {
        Some(value) => serde_json::from_str(value)
            .map_err(|error| refused(format!("Invalid seal: {}", error), 1))?,
        None => Seal::default(),
    };

    let clock = CliClock::from_args(args).map_err(|error| refused(error, 1))?;
    let id = args.flags.get("id").cloned().unwrap_or_else(|| slug(name));
    let timestamp = clock.timestamp();
    let rule = TechnologyManagementRule {
        id: id.clone(),
        name: name.clone(),
        enabled: true,
        target,
        schedule,
        penalty,
        pass,
        seal,
        created_at: timestamp.clone(),
        updated_at: timestamp,
    };
    rule.validate().map_err(|error| refused(error.to_string(), 1))?;

    let mut store = load(args)?;
    if store.rules.iter().any(|existing| existing.id == id) {
        return Err(refused(format!("A rule with the id {} already exists.", id), 1));
    }
    store.rules.push(rule);
    save(args, &store)?;

    if args.flags.contains_key("json") {
        emit_json(output_json::message(&format!("Created rule {}.", id)), out)
    } else {
        emit_text(
            &format!("Created rule {} with target {}.", id, output_human::target_name(&store.rules.last().unwrap().target)),
            out,
        )
    }
}
