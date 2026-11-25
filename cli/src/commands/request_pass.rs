use std::io::Write;
use std::path::PathBuf;

use chrono::Datelike;
use leko_core::domain::pass_active;
use leko_core::models::Pass;
use leko_core::services::{
    domain_state, load_pass_state, pass_state_path, record_pass, save_pass_state,
    wait_for_requirement, RealSleeper, Sleeper,
};

use crate::arguments::CliArgs;
use crate::clock::CliClock;
use crate::commands::{emit_json, find_rule, load, refused, store_path, CommandResult};
use crate::services::addon;
use crate::services::output_human;

pub fn run_with(args: &CliArgs, out: &mut dyn Write, sleeper: &mut dyn Sleeper) -> CommandResult {
    if !addon::check_pinned(args).map_err(|error| refused(error, 1))? {
        return Err(refused("the custom verifier was not satisfied", 1));
    }
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
    let requirement = match &pass {
        Pass::EarnPass { requirement, .. } => requirement,
        Pass::NoPass => return Err(refused("this rule has no pass", 1)),
    };
    wait_for_requirement(requirement, sleeper).map_err(|error| refused(error.to_string(), 1))?;

    let clock = CliClock::from_args(args).map_err(|error| refused(error, 1))?;
    let local = clock.moment.utc.with_timezone(&clock.moment.zone);
    let window_key = format!("{:04}-{:02}-{:02}", local.year(), local.month(), local.day());
    let path = args
        .flags
        .get("pass-state")
        .map(PathBuf::from)
        .unwrap_or_else(|| pass_state_path(&store_path(args)));
    let mut state = load_pass_state(&path).map_err(|error| refused(error.to_string(), 1))?;
    record_pass(&mut state, rule, &clock.moment, window_key)
        .map_err(|error| refused(error.to_string(), 1))?;
    save_pass_state(&path, &state).map_err(|error| refused(error.to_string(), 1))?;

    let domain = domain_state(&state, id);
    let active = pass_active(&pass, &domain, &clock.moment);
    let earned_at = clock.timestamp();
    if args.flags.contains_key("json") {
        emit_json(
            serde_json::json!({
                "pass": output_human::pass_name(&pass),
                "active": active,
                "earned_at": earned_at,
                "waited_ms": sleeper.waited_ms(),
            }),
            out,
        )
    } else {
        writeln!(out, "Pass: {}", output_human::pass_name(&pass))
            .map_err(|error| refused(error.to_string(), 1))?;
        writeln!(out, "Active: {}", active)
            .map_err(|error| refused(error.to_string(), 1))?;
        writeln!(out, "Earned At: {}", earned_at)
            .map_err(|error| refused(error.to_string(), 1))?;
        writeln!(out, "Waited Ms: {}", sleeper.waited_ms())
            .map_err(|error| refused(error.to_string(), 1))
    }
}

pub fn run(args: &CliArgs, out: &mut dyn Write) -> CommandResult {
    run_with(args, out, &mut RealSleeper)
}
