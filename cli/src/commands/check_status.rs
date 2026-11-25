use std::io::Write;

use leko_core::domain::{decide, UsageState};

use crate::arguments::CliArgs;
use crate::clock::CliClock;
use crate::commands::{find_rule, load, refused, store_path, CommandResult};
use crate::services::enforcement_loop::pass_state_for;
use crate::services::{output_human, output_json};

fn parse_u32(args: &CliArgs, name: &str) -> Result<u32, (String, i32)> {
    match args.flags.get(name) {
        None => Ok(0),
        Some(value) => value
            .parse::<u32>()
            .map_err(|_| refused(format!("The flag --{} needs a whole number.", name), 2)),
    }
}

pub fn run(args: &CliArgs, out: &mut dyn Write) -> CommandResult {
    let id = args
        .flags
        .get("id")
        .ok_or_else(|| refused("The flag --id needs a value.", 2))?;
    let store = load(args)?;
    let rule = find_rule(&store, id)?;
    let clock = CliClock::from_args(args).map_err(|error| refused(error, 1))?;
    let usage = UsageState {
        period_key: args.flags.get("usage-period-key").cloned().unwrap_or_default(),
        used_minutes: parse_u32(args, "used-minutes")?,
        sessions: parse_u32(args, "sessions")?,
    };
    let pass = pass_state_for(args, &store_path(args), id);
    let answer = decide(rule, &clock.moment, &usage, &pass);
    let pass_active = pass.earned_at.is_some()
        && leko_core::domain::pass_active(&rule.pass, &pass, &clock.moment);
    if args.flags.contains_key("json") {
        emit_pair(output_json::decision(&answer), rule.name.as_str(), pass_active, out)
    } else {
        writeln!(out, "Rule: {}", rule.name).map_err(|error| refused(error.to_string(), 1))?;
        writeln!(out, "Pass Active: {}", pass_active)
            .map_err(|error| refused(error.to_string(), 1))?;
        output_human::decision(&answer, out).map_err(|message| refused(message, 1))
    }
}

fn emit_pair(
    value: serde_json::Value,
    name: &str,
    pass_active: bool,
    out: &mut dyn Write,
) -> CommandResult {
    let mut object = value;
    if let Some(map) = object.as_object_mut() {
        map.insert("rule".to_string(), serde_json::Value::String(name.to_string()));
        map.insert("pass".to_string(), serde_json::json!({"active": pass_active}));
    }
    writeln!(out, "{}", object).map_err(|error| refused(error.to_string(), 1))
}
