use std::io::Write;

use chrono::Datelike;
use leko_core::domain::{pass_active, PassState};
use leko_core::models::Pass;

use crate::arguments::CliArgs;
use crate::clock::CliClock;
use crate::commands::{emit_json, find_rule, load, refused, CommandResult};
use crate::services::output_human;

pub fn run(args: &CliArgs, out: &mut dyn Write) -> CommandResult {
    let id = args
        .flags
        .get("id")
        .ok_or_else(|| refused("The flag --id needs a value.", 2))?;
    let store = load(args)?;
    let rule = find_rule(&store, id)?;
    let pass: Pass = match args.flags.get("pass") {
        Some(value) => serde_json::from_str(value)
            .map_err(|error| refused(format!("Invalid pass: {}", error), 1))?,
        None => rule.pass.clone(),
    };
    let clock = CliClock::from_args(args).map_err(|error| refused(error, 1))?;
    let local = clock.moment.utc.with_timezone(&clock.moment.zone);
    let window_key = format!("{:04}-{:02}-{:02}", local.year(), local.month(), local.day());
    let state = PassState {
        earned_at: Some(clock.timestamp()),
        used_in_window: 0,
        window_key,
    };
    let active = pass_active(&pass, &state, &clock.moment);
    if args.flags.contains_key("json") {
        emit_json(
            serde_json::json!({
                "pass": output_human::pass_name(&pass),
                "active": active,
                "earned_at": clock.timestamp(),
            }),
            out,
        )
    } else {
        writeln!(out, "Pass: {}", output_human::pass_name(&pass))
            .map_err(|error| refused(error.to_string(), 1))?;
        writeln!(out, "Active: {}", active).map_err(|error| refused(error.to_string(), 1))?;
        writeln!(out, "Earned At: {}", clock.timestamp())
            .map_err(|error| refused(error.to_string(), 1))
    }
}
