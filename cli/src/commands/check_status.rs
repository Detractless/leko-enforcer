use std::io::Write;

use leko_core::countdowns::{compute_countdowns, rule_countdowns, CountdownItem};
use leko_core::domain::{budget_status, decide_tracked, UsageBudgetStatus};
use leko_core::services::{load_pass_state, load_usage_state, PassStateFile, UsageStateFile};

use crate::arguments::CliArgs;
use crate::clock::CliClock;
use crate::commands::{find_rule, load, refused, store_path, CommandResult};
use crate::services::enforcement_loop::pass_state_for;
use crate::services::usage_tracker::{pass_state_path_for, resolve_status_usage, usage_state_path_for};
use crate::services::{output_human, output_json};

pub fn run(args: &CliArgs, out: &mut dyn Write) -> CommandResult {
    let id = args
        .flags
        .get("id")
        .ok_or_else(|| refused("The flag --id needs a value.", 2))?;
    let store = load(args)?;
    let rule = find_rule(&store, id)?;
    let clock = CliClock::from_args(args).map_err(|error| refused(error, 1))?;
    let store_file = store_path(args);
    let (usage, active_seconds, session_open) =
        resolve_status_usage(args, &store_file, rule, &clock.moment)?;
    let pass = pass_state_for(args, &store_file, id);
    let answer = decide_tracked(rule, &clock.moment, &usage, session_open, &pass);
    let pass_active = pass.earned_at.is_some()
        && leko_core::domain::pass_active(&rule.pass, &pass, &clock.moment);
    let budget = budget_status(
        rule.schedule.usage_based.as_ref(),
        &usage,
        active_seconds,
        session_open,
        &clock.moment,
    );
    let usage_file = load_usage_safe(args, &store_file)?;
    let pass_file = load_pass_safe(args, &store_file)?;
    let countdowns = rule_countdowns(rule, &usage_file, &pass_file, &clock.moment);
    if args.flags.contains_key("json") {
        emit_pair(output_json::decision(&answer), rule.name.as_str(), pass_active, budget, countdowns, out)
    } else {
        writeln!(out, "Rule: {}", rule.name).map_err(|error| refused(error.to_string(), 1))?;
        writeln!(out, "Pass Active: {}", pass_active)
            .map_err(|error| refused(error.to_string(), 1))?;
        if let Some(info) = &budget {
            writeln!(out, "Usage Limit: {}", info.kind)
                .map_err(|error| refused(error.to_string(), 1))?;
            if let Some(rem) = info.remaining_seconds {
                writeln!(out, "Used Seconds: {}", info.used_seconds)
                    .map_err(|error| refused(error.to_string(), 1))?;
                writeln!(out, "Remaining Seconds: {}", rem)
                    .map_err(|error| refused(error.to_string(), 1))?;
            }
            if let Some(rem_sessions) = info.remaining_sessions {
                writeln!(out, "Sessions Used: {}", info.sessions_used)
                    .map_err(|error| refused(error.to_string(), 1))?;
                writeln!(out, "Remaining Sessions: {}", rem_sessions)
                    .map_err(|error| refused(error.to_string(), 1))?;
            }
        }
        if let Some(first) = countdowns.first() {
            writeln!(out, "Next Countdown: {} ({}, {}s)", first.kind, first.transition, first.remaining_seconds)
                .map_err(|error| refused(error.to_string(), 1))?;
        }
        output_human::decision(&answer, out).map_err(|message| refused(message, 1))
    }
}

pub fn countdowns(args: &CliArgs, out: &mut dyn Write) -> CommandResult {
    let store = load(args)?;
    let clock = CliClock::from_args(args).map_err(|error| refused(error, 1))?;
    let store_file = store_path(args);
    let usage_file = load_usage_safe(args, &store_file)?;
    let pass_file = load_pass_safe(args, &store_file)?;
    let report = compute_countdowns(&store.rules, &usage_file, &pass_file, &clock.moment);
    if args.flags.contains_key("json") {
        let value = serde_json::to_value(&report).map_err(|error| refused(error.to_string(), 1))?;
        return writeln!(out, "{}", value).map_err(|error| refused(error.to_string(), 1));
    }
    if report.countdowns.is_empty() {
        return writeln!(out, "Countdowns: None")
            .map_err(|error| refused(error.to_string(), 1));
    }
    writeln!(out, "Countdowns: {}", report.countdowns.len())
        .map_err(|error| refused(error.to_string(), 1))?;
    for item in &report.countdowns {
        writeln!(
            out,
            "Rule: {} | Type: {} | Transition: {} | Remaining Seconds: {}",
            item.rule, item.kind, item.transition, item.remaining_seconds
        )
        .map_err(|error| refused(error.to_string(), 1))?;
    }
    Ok(())
}

fn load_usage_safe(args: &CliArgs, store: &std::path::Path) -> Result<UsageStateFile, (String, i32)> {
    let path = usage_state_path_for(args, store);
    match load_usage_state(&path) {
        Ok(loaded) => Ok(loaded),
        Err(_) if args.flags.get("usage-state").is_none() => Ok(UsageStateFile::default()),
        Err(error) => Err(refused(error.to_string(), 1)),
    }
}

fn load_pass_safe(args: &CliArgs, store: &std::path::Path) -> Result<PassStateFile, (String, i32)> {
    let path = pass_state_path_for(args, store);
    match load_pass_state(&path) {
        Ok(loaded) => Ok(loaded),
        Err(_) if args.flags.get("pass-state").is_none() => Ok(PassStateFile::default()),
        Err(error) => Err(refused(error.to_string(), 1)),
    }
}

fn emit_pair(
    value: serde_json::Value,
    name: &str,
    pass_active: bool,
    budget: Option<UsageBudgetStatus>,
    countdowns: Vec<CountdownItem>,
    out: &mut dyn Write,
) -> CommandResult {
    let mut object = value;
    if let Some(map) = object.as_object_mut() {
        map.insert("rule".to_string(), serde_json::Value::String(name.to_string()));
        map.insert("pass".to_string(), serde_json::json!({"active": pass_active}));
        if let Some(info) = budget {
            if let Ok(val) = serde_json::to_value(info) {
                map.insert("usage".to_string(), val);
            }
        }
        if let Ok(val) = serde_json::to_value(&countdowns) {
            map.insert("countdowns".to_string(), val);
        }
    }
    writeln!(out, "{}", object).map_err(|error| refused(error.to_string(), 1))
}
