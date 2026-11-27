use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};

use chrono_tz::Tz;
use leko_core::clock::Moment;
use leko_core::domain::{decide_tracked, PassState, UsageState};
use leko_core::enforcement::{
    judge_guard, BrowserId, DefaultHandshake, GuardAction, GuardConfig, Heartbeat,
};
use leko_core::models::Target;
use leko_core::services::{
    domain_usage, load_pass_state, load_usage_state, UsageStateFile,
};
use leko_core::store::load;

use crate::arguments::CliArgs;
use crate::clock::CliClock;
use crate::commands::{refused, CommandResult};
use crate::services::os_executor::{ObservedApp, OsExecutor, SystemExecutor};
use crate::services::usage_tracker::{
    pass_state_path_for, save_usage_safe, update_tick,
    usage_state_path_for, write_pill_state,
};

const USAGE: &str = "Usage: leko enforce STORE.json [--once] [--dry-run] [--interval-ms N] [--heartbeats PATH] [--pill-events PATH]";

const PROTECTED: &[&str] = &[
    "com.apple.finder", "com.apple.dock", "loginwindow", "com.apple.systemuiserver",
];

struct Options {
    once: bool,
    dry_run: bool,
    interval_ms: u64,
    debounce_seconds: u64,
}

fn parse_u64_flag(args: &CliArgs, name: &str, default: u64, label: &str) -> Result<u64, (String, i32)> {
    match args.flags.get(name) {
        None => Ok(default),
        Some(value) => value
            .parse::<u64>()
            .map_err(|_| refused(format!("The {} must be a whole number.", label), 2)),
    }
}

fn options(args: &CliArgs) -> Result<Options, (String, i32)> {
    Ok(Options {
        once: args.flags.contains_key("once"),
        dry_run: args.flags.contains_key("dry-run"),
        interval_ms: parse_u64_flag(args, "interval-ms", 1000, "interval")?,
        debounce_seconds: parse_u64_flag(args, "session-debounce-seconds", 5, "debounce seconds")?,
    })
}

fn store_path(args: &CliArgs) -> Result<PathBuf, (String, i32)> {
    args.positional
        .first()
        .map(PathBuf::from)
        .ok_or_else(|| refused(USAGE.to_string(), 2))
}

fn usage_override(args: &CliArgs) -> Option<UsageState> {
    if !args.flags.contains_key("used-minutes")
        && !args.flags.contains_key("sessions")
        && !args.flags.contains_key("usage-period-key")
    {
        return None;
    }
    let number = |name: &str| -> u32 {
        args.flags.get(name).and_then(|value| value.parse().ok()).unwrap_or(0)
    };
    Some(UsageState {
        period_key: args.flags.get("usage-period-key").cloned().unwrap_or_default(),
        used_minutes: number("used-minutes"),
        sessions: number("sessions"),
    })
}

pub fn pass_state_for(args: &CliArgs, store: &Path, rule_id: &str) -> PassState {
    let number = |name: &str| -> u32 {
        args.flags.get(name).and_then(|value| value.parse().ok()).unwrap_or(0)
    };
    if let Some(earned) = args.flags.get("earned-at") {
        return PassState {
            earned_at: Some(earned.clone()),
            used_in_window: number("pass-used"),
            window_key: args.flags.get("pass-window-key").cloned().unwrap_or_default(),
        };
    }
    let path = pass_state_path_for(args, store);
    let saved = load_pass_state(&path).unwrap_or_default();
    leko_core::services::domain_state(&saved, rule_id)
}

fn is_protected(app: &ObservedApp) -> bool {
    if app.pid == std::process::id() as i32 {
        return true;
    }
    app.bundle
        .as_deref()
        .map(|bundle| PROTECTED.iter().any(|name| bundle.contains(name)))
        .unwrap_or(true)
}

fn terminate_apps<E: OsExecutor>(
    executor: &mut E,
    apps: &[String],
    allow_only: bool,
    dry_run: bool,
    err: &mut dyn Write,
    blocked: &mut Option<(String, String)>,
) -> Result<(), String> {
    for app in executor.running_apps()? {
        if is_protected(&app) {
            continue;
        }
        let bundle = app.bundle.clone().unwrap_or_default();
        let selected = if allow_only { !apps.contains(&bundle) } else { apps.contains(&bundle) };
        if !selected {
            continue;
        }
        if dry_run {
            let _ = writeln!(err, "Dry run: would terminate {}.", bundle);
        } else if let Err(problem) = executor.terminate_app(&app) {
            let _ = writeln!(err, "Could not terminate {}: {}; will retry.", bundle, problem);
        } else {
            *blocked = Some(("app".to_string(), bundle));
        }
    }
    Ok(())
}

fn judge_browsers<E: OsExecutor>(
    executor: &mut E,
    now_ms: i64,
    first_unhealthy: &mut HashMap<BrowserId, i64>,
    dry_run: bool,
    err: &mut dyn Write,
    blocked: &mut Option<(String, String)>,
) -> Result<(), String> {
    let browsers = executor.browsers()?;
    let beats: HashMap<BrowserId, Heartbeat> = executor.heartbeats();
    let config = GuardConfig::default();
    for (browser, action) in judge_guard(
        &DefaultHandshake, &browsers, &beats, now_ms, true, first_unhealthy, &config,
    ) {
        match action {
            GuardAction::Warn if !dry_run => executor.warn_browser(&browser),
            GuardAction::Close if !dry_run => {
                executor.close_browser(&browser);
                *blocked = Some(("website".to_string(), browser.bundle_id.clone()));
            }
            GuardAction::Warn => {
                let _ = writeln!(err, "Dry run: would warn {}.", browser.bundle_id);
            }
            GuardAction::Close => {
                let _ = writeln!(err, "Dry run: would close {}.", browser.bundle_id);
            }
            GuardAction::None => {}
        }
    }
    Ok(())
}

fn tick_moment<E: OsExecutor>(args: &CliArgs, executor: &mut E) -> Result<(Moment, bool), (String, i32)> {
    if let Some(raw) = executor.tick_moment_override() {
        let zone_name = args.flags.get("zone").map(String::as_str).unwrap_or("UTC");
        let zone: Tz = zone_name
            .parse()
            .map_err(|_| refused(format!("The timezone {} is not known.", zone_name), 1))?;
        let moment = Moment::parse(&raw, zone)
            .map_err(|error| refused(format!("The timestamp {} is not valid: {}", raw, error), 1))?;
        return Ok((moment, true));
    }
    let clock = CliClock::from_args(args).map_err(|error| refused(error, 1))?;
    Ok((clock.moment, args.flags.contains_key("at")))
}

pub fn enforce_with<E: OsExecutor>(
    args: &CliArgs,
    out: &mut dyn Write,
    err: &mut dyn Write,
    executor: &mut E,
) -> CommandResult {
    let options = options(args)?;
    let path = store_path(args)?;
    let mut store = load(&path).map_err(|error| refused(error.to_string(), 1))?;
    let usage_path = usage_state_path_for(args, &path);
    let pass_path = pass_state_path_for(args, &path);
    let mut first_unhealthy: HashMap<BrowserId, i64> = HashMap::new();
    let mut last_problem: Option<String> = None;
    let mut last_usage_problem: Option<String> = None;
    let mut usage_file = UsageStateFile::default();
    let mut ticks = 0usize;

    loop {
        if executor.shutdown_requested() {
            break;
        }
        match load(&path) {
            Ok(next) => {
                store = next;
                last_problem = None;
            }
            Err(problem) => {
                let message = problem.to_string();
                if last_problem.as_deref() != Some(message.as_str()) {
                    let _ = writeln!(err, "Store reload failed; keeping last valid state: {}", message);
                    last_problem = Some(message);
                }
            }
        }

        let usage_corrupt = match load_usage_state(&usage_path) {
            Ok(loaded) => {
                usage_file = loaded;
                last_usage_problem = None;
                false
            }
            Err(problem) => {
                let message = problem.to_string();
                if last_usage_problem.as_deref() != Some(message.as_str()) {
                    let _ = writeln!(err, "Usage state unreadable; using safe defaults: {}", message);
                    last_usage_problem = Some(message);
                }
                true
            }
        };

        let (moment, injected) = tick_moment(args, executor)?;
        let activity = match executor.observe_foreground() {
            Ok(found) => found,
            Err(problem) => {
                let _ = writeln!(err, "Foreground observation failed: {}", problem);
                None
            }
        };
        let mut pass_file = load_pass_state(&pass_path).unwrap_or_default();
        let max_gap = if injected { 3600 } else { (options.interval_ms / 1000).max(1).saturating_mul(5).max(10) };

        if !store.rules.is_empty() {
            update_tick(
                &mut usage_file, &mut pass_file, &pass_path, &store.rules, &moment,
                activity.as_ref(), options.debounce_seconds, max_gap,
            );
            if !usage_corrupt {
                if let Err(problem) = save_usage_safe(&usage_path, &usage_file) {
                    let _ = writeln!(err, "Could not save usage state: {}", problem);
                }
            }
        }

        let manual_usage = usage_override(args);
        let mut website_locked = false;
        let mut blocked: Option<(String, String)> = None;

        for rule in &store.rules {
            let pass = pass_state_for(args, &path, &rule.id);
            let (usage, session_open) = match &manual_usage {
                Some(override_state) => (override_state.clone(), false),
                None => {
                    let (u, _, open) = domain_usage(&usage_file, &rule.id);
                    (u, open)
                }
            };
            let answer = decide_tracked(rule, &moment, &usage, session_open, &pass);
            if !answer.enforced {
                continue;
            }
            match &rule.target {
                Target::SpecificApps { apps } => {
                    terminate_apps(executor, apps, false, options.dry_run, err, &mut blocked)
                        .map_err(|message| refused(message, 1))?;
                }
                Target::AllowOnlyApps { allowed_apps } => {
                    terminate_apps(executor, allowed_apps, true, options.dry_run, err, &mut blocked)
                        .map_err(|message| refused(message, 1))?;
                }
                Target::SpecificWebsites { .. }
                | Target::TopicDetectedWebsites { .. }
                | Target::AllowOnlyWebsites { .. } => website_locked = true,
                Target::WholeDevice { action } => {
                    if options.dry_run {
                        let _ = writeln!(err, "Dry run: would perform {:?}.", action);
                    } else if let Err(problem) = executor.device_action(*action) {
                        let _ = writeln!(err, "Could not perform {:?}: {}; will retry.", action, problem);
                    } else {
                        blocked = Some(("device".to_string(), format!("{:?}", action)));
                    }
                }
            }
        }

        if website_locked {
            judge_browsers(executor, moment.utc.timestamp_millis(), &mut first_unhealthy, options.dry_run, err, &mut blocked)
                .map_err(|message| refused(message, 1))?;
        }

        if !options.dry_run {
            let report = leko_core::countdowns::compute_countdowns(
                &store.rules, &usage_file, &pass_file, &moment,
            );
            let timer = report.countdowns.into_iter().find(|item| {
                if item.kind != "Time Limit" {
                    return true;
                }
                store.rules.iter().find(|r| r.id == item.rule_id).is_some_and(|r| {
                    crate::services::usage_tracker::target_or_visible_matches(
                        &r.target, activity.as_ref(),
                    )
                })
            }).map(|item| item.remaining_seconds);
            let blocked_ref = blocked.as_ref().map(|(k, t)| (k.as_str(), t.as_str()));
            write_pill_state(args, blocked_ref, timer, &moment.utc.to_rfc3339());
        }

        ticks += 1;
        if options.once {
            break;
        }
        executor.sleep(options.interval_ms);
    }

    writeln!(out, "Enforcement ticks: {}", ticks).map_err(|error| refused(error.to_string(), 1))
}

pub fn enforce(args: &CliArgs, out: &mut dyn Write, err: &mut dyn Write) -> CommandResult {
    let heartbeats = args.flags.get("heartbeats").map(PathBuf::from);
    let eval_log = args.flags.get("eval-log").map(PathBuf::from).or_else(|| {
        std::env::var_os("WAE_RUNTIME_DIR")
            .map(PathBuf::from)
            .map(|dir| dir.join("evaluations.jsonl"))
    });
    let mut executor =
        SystemExecutor::new_with_sources(heartbeats, eval_log).map_err(|message| refused(message, 1))?;
    enforce_with(args, out, err, &mut executor)
}
