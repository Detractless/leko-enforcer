use std::collections::HashMap;
use std::io::Write;

use chrono::Utc;
use std::path::{Path, PathBuf};

use leko_core::domain::{decide, PassState, UsageState};
use leko_core::enforcement::{
    judge_guard, BrowserId, DefaultHandshake, GuardAction, GuardConfig, Heartbeat,
};
use leko_core::models::Target;
use leko_core::store::load;

use crate::arguments::CliArgs;
use crate::clock::CliClock;
use crate::commands::{refused, CommandResult};
use crate::services::os_executor::{ObservedApp, OsExecutor, SystemExecutor};

const USAGE: &str = "Usage: leko enforce STORE.json [--once] [--dry-run] [--interval-ms N] [--heartbeats PATH] [--pill-events PATH]";

const PROTECTED: &[&str] = &[
    "com.apple.finder", "com.apple.dock", "loginwindow", "com.apple.systemuiserver",
];

struct Options {
    once: bool,
    dry_run: bool,
    interval_ms: u64,
}

fn options(args: &CliArgs) -> Result<Options, (String, i32)> {
    let interval_ms = match args.flags.get("interval-ms") {
        None => 1000,
        Some(value) => value.parse::<u64>()
            .map_err(|_| refused("The interval must be a whole number.", 2))?,
    };
    Ok(Options {
        once: args.flags.contains_key("once"),
        dry_run: args.flags.contains_key("dry-run"),
        interval_ms,
    })
}

fn store_path(args: &CliArgs) -> Result<PathBuf, (String, i32)> {
    args.positional.first()
        .map(PathBuf::from)
        .ok_or_else(|| refused(USAGE.to_string(), 2))
}

fn usage_state(args: &CliArgs) -> UsageState {
    let number = |name: &str| -> u32 {
        args.flags.get(name).and_then(|value| value.parse().ok()).unwrap_or(0)
    };
    UsageState {
        period_key: args.flags.get("usage-period-key").cloned().unwrap_or_default(),
        used_minutes: number("used-minutes"),
        sessions: number("sessions"),
    }
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
    let path = args.flags.get("pass-state")
        .map(PathBuf::from)
        .unwrap_or_else(|| leko_core::services::pass_state_path(store));
    let saved = leko_core::services::load_pass_state(&path).unwrap_or_default();
    leko_core::services::domain_state(&saved, rule_id)
}

fn write_pill_event(args: &CliArgs, kind: &str, target: &str) {
    let Some(path) = args.flags.get("pill-events") else {
        return;
    };
    let value = serde_json::json!({
        "version": 1,
        "event": "blocked",
        "kind": kind,
        "target": target,
        "at": Utc::now().to_rfc3339(),
    });
    let Ok(bytes) = serde_json::to_vec(&value) else {
        return;
    };
    let path = PathBuf::from(path);
    let temp = path.with_extension("tmp");
    if std::fs::write(&temp, bytes).is_ok() {
        let _ = std::fs::rename(temp, path);
    }
}

fn is_protected(app: &ObservedApp) -> bool {
    if app.pid == std::process::id() as i32 {
        return true;
    }
    app.bundle.as_deref()
        .map(|bundle| PROTECTED.iter().any(|name| bundle.contains(name)))
        .unwrap_or(true)
}

fn terminate_apps<E: OsExecutor>(
    args: &CliArgs,
    executor: &mut E,
    apps: &[String],
    allow_only: bool,
    dry_run: bool,
    err: &mut dyn Write,
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
            write_pill_event(args, "app", &bundle);
        }
    }
    Ok(())
}

fn judge_browsers<E: OsExecutor>(
    args: &CliArgs,
    executor: &mut E,
    now_ms: i64,
    first_unhealthy: &mut HashMap<BrowserId, i64>,
    dry_run: bool,
    err: &mut dyn Write,
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
                write_pill_event(args, "website", &browser.bundle_id);
            }
            GuardAction::Warn => { let _ = writeln!(err, "Dry run: would warn {}.", browser.bundle_id); }
            GuardAction::Close => { let _ = writeln!(err, "Dry run: would close {}.", browser.bundle_id); }
            GuardAction::None => {}
        }
    }
    Ok(())
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
    let mut first_unhealthy: HashMap<BrowserId, i64> = HashMap::new();
    let mut last_problem: Option<String> = None;
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

        let clock = CliClock::from_args(args).map_err(|error| refused(error, 1))?;
        let usage = usage_state(args);
        let mut website_locked = false;

        for rule in &store.rules {
            let pass = pass_state_for(args, &path, &rule.id);
            let answer = decide(rule, &clock.moment, &usage, &pass);
            if !answer.enforced {
                continue;
            }
            match &rule.target {
                Target::SpecificApps { apps } => {
                    terminate_apps(args, executor, apps, false, options.dry_run, err)
                        .map_err(|message| refused(message, 1))?;
                }
                Target::AllowOnlyApps { allowed_apps } => {
                    terminate_apps(args, executor, allowed_apps, true, options.dry_run, err)
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
                        write_pill_event(args, "device", &format!("{:?}", action));
                    }
                }
            }
        }

        if website_locked {
            judge_browsers(args, executor, clock.now_ms(), &mut first_unhealthy, options.dry_run, err)
                .map_err(|message| refused(message, 1))?;
        }

        ticks += 1;
        if options.once {
            break;
        }
        executor.sleep(options.interval_ms);
    }

    writeln!(out, "Enforcement ticks: {}", ticks)
        .map_err(|error| refused(error.to_string(), 1))
}

pub fn enforce(args: &CliArgs, out: &mut dyn Write, err: &mut dyn Write) -> CommandResult {
    let heartbeats = args.flags.get("heartbeats").map(PathBuf::from);
    let mut executor = SystemExecutor::new(heartbeats).map_err(|message| refused(message, 1))?;
    enforce_with(args, out, err, &mut executor)
}
