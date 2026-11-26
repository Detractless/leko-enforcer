use std::io::Write;

use chrono::Datelike;
use leko_core::domain::pass_active;
use leko_core::models::{Pass, PassRequirement};
use leko_core::services::{
    domain_state, load_pass_state, load_usage_state, record_pass, reset_engagement,
    save_pass_state, save_usage_state, wait_for_requirement, RealSleeper, Sleeper,
};

use crate::arguments::CliArgs;
use crate::clock::CliClock;
use crate::commands::{emit_json, find_rule, load, refused, store_path, CommandResult};
use crate::services::addon;
use crate::services::output_human;
use crate::services::usage_tracker::{pass_state_path_for, usage_state_path_for};

pub fn run_with(args: &CliArgs, out: &mut dyn Write, sleeper: &mut dyn Sleeper) -> CommandResult {
    if !addon::check_pinned(args).map_err(|error| refused(error, 1))? {
        return Err(refused("the custom verifier was not satisfied", 1));
    }
    let elapsed_override = match args.flags.get("elapsed-seconds") {
        None => None,
        Some(raw) => Some(
            raw.parse::<u64>()
                .map_err(|_| refused("The flag --elapsed-seconds needs a whole number.", 2))?,
        ),
    };
    let id = args
        .flags
        .get("id")
        .ok_or_else(|| refused("The flag --id needs a value.", 2))?;
    let store = load(args)?;
    let rule = find_rule(&store, id)?;
    let pass: Pass = match args.flags.get("pass") {
        Some(value) => serde_json::from_str(value)
            .map_err(|error| refused(format!("Invalid pass: {}", error), 1))?,
        None if args.flags.contains_key("engagement-pass") => {
            crate::commands::create_rule::parse_engagement_pass(args)?
        }
        None => rule.pass.clone(),
    };
    let requirement = match &pass {
        Pass::EarnPass { requirement, .. } => requirement,
        Pass::NoPass => return Err(refused("this rule has no pass", 1)),
    };

    let store_file = store_path(args);
    let usage_path = usage_state_path_for(args, &store_file);
    let mut usage_state = load_usage_state(&usage_path).unwrap_or_default();
    if let PassRequirement::EngagementPass { required_seconds, .. } = requirement {
        let tracked = usage_state.rules.get(id).map(|e| e.engagement_seconds).unwrap_or(0);
        let total = tracked.saturating_add(elapsed_override.unwrap_or(0));
        if total < *required_seconds {
            return Err(refused(
                format!("Engagement Pass requires {} active focus seconds ({} completed).", required_seconds, total),
                1,
            ));
        }
    }

    wait_for_requirement(requirement, sleeper).map_err(|error| refused(error.to_string(), 1))?;

    let clock = CliClock::from_args(args).map_err(|error| refused(error, 1))?;
    let local = clock.moment.utc.with_timezone(&clock.moment.zone);
    let window_key = format!("{:04}-{:02}-{:02}", local.year(), local.month(), local.day());
    let path = pass_state_path_for(args, &store_file);
    let mut state = load_pass_state(&path).map_err(|error| refused(error.to_string(), 1))?;
    let mut eval_rule = rule.clone();
    eval_rule.pass = pass.clone();
    record_pass(&mut state, &eval_rule, &clock.moment, window_key)
        .map_err(|error| refused(error.to_string(), 1))?;
    save_pass_state(&path, &state).map_err(|error| refused(error.to_string(), 1))?;

    if matches!(requirement, PassRequirement::EngagementPass { .. }) && usage_state.rules.contains_key(id) {
        reset_engagement(&mut usage_state, id);
        let _ = save_usage_state(&usage_path, &usage_state);
    }

    let domain = domain_state(&state, id);
    let active = pass_active(&pass, &domain, &clock.moment);
    let earned_at = clock.timestamp();
    let req_name = output_human::requirement_name(requirement);
    if args.flags.contains_key("json") {
        emit_json(
            serde_json::json!({
                "pass": output_human::pass_name(&pass),
                "requirement": req_name,
                "active": active,
                "earned_at": earned_at,
                "waited_ms": sleeper.waited_ms(),
            }),
            out,
        )
    } else {
        writeln!(out, "Pass: {}", output_human::pass_name(&pass))
            .map_err(|error| refused(error.to_string(), 1))?;
        writeln!(out, "Requirement: {}", req_name)
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
