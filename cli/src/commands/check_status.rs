use std::io::Write;

use leko_core::domain::{decide, PassState, UsageState};

use crate::arguments::CliArgs;
use crate::clock::CliClock;
use crate::commands::{find_rule, load, refused, CommandResult};
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
    let pass = PassState {
        earned_at: args.flags.get("earned-at").cloned(),
        used_in_window: parse_u32(args, "pass-used")?,
        window_key: args.flags.get("pass-window-key").cloned().unwrap_or_default(),
    };
    let answer = decide(rule, &clock.moment, &usage, &pass);
    if args.flags.contains_key("json") {
        emit_pair(output_json::decision(&answer), rule.name.as_str(), out)
    } else {
        writeln!(out, "Rule: {}", rule.name).map_err(|error| refused(error.to_string(), 1))?;
        output_human::decision(&answer, out).map_err(|message| refused(message, 1))
    }
}

fn emit_pair(value: serde_json::Value, name: &str, out: &mut dyn Write) -> CommandResult {
    let mut object = value;
    if let Some(map) = object.as_object_mut() {
        map.insert("rule".to_string(), serde_json::Value::String(name.to_string()));
    }
    writeln!(out, "{}", object).map_err(|error| refused(error.to_string(), 1))
}
