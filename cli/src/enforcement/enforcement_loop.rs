//! Applies the core's decisions to an injected list of running applications.
use super::macos_applications::Application;
use crate::status::settings_file::read_settings_and_bytes;
use crate::status::zone_choice::choose_zone;
use leko_core::config_time::zone_time::ZoneTime;
use leko_core::decision::block_cover::Target;
use leko_core::decision::decide_target::decide;
use leko_core::models::specs::ReadSet;
use std::collections::HashSet;
use std::io::Write;

pub fn protected() -> HashSet<String> {
    ["com.apple.finder", "com.apple.dock", "com.apple.loginwindow", "com.apple.systemuiserver"]
        .into_iter().map(str::to_string).collect()
}

pub fn judge_apps<F: FnMut(&Application) -> Result<(), String>>(
    set: &ReadSet, apps: &[Application], now: i64, zone: &ZoneTime, own_pid: i32,
    mut terminate: F, err: &mut dyn Write,
) {
    let protected = protected();
    for app in apps {
        if app.pid == own_pid { continue; }
        let Some(bundle) = app.bundle.as_deref() else {
            let _ = writeln!(err, "Skipped process {}: no bundle ID.", app.pid);
            continue;
        };
        if protected.contains(&bundle.to_ascii_lowercase()) { continue; }
        let decision = decide(set, Target::App(bundle), now, zone, &protected);
        for _ in decision.not_judged {
            let _ = writeln!(err, "Not judged for {bundle}: a usage-count rule needs data that is not available.");
        }
        if decision.closed {
            if let Err(problem) = terminate(app) {
                let _ = writeln!(err, "Could not terminate {bundle} (PID {}): {problem}; will retry.", app.pid);
            }
        }
    }
}

pub fn run<C, Z, A, T, M, S>(path: &str, mut clock: C, mut zone_name: Z, mut apps: A, mut terminate: T, own_pid: i32, mut monotonic_ms: M, mut sleep: S, err: &mut dyn Write) -> i32
where C: FnMut() -> Result<i64, String>, Z: FnMut() -> Option<String>,
      A: FnMut() -> Result<Vec<Application>, String>, T: FnMut(&Application) -> Result<(), String>, M: FnMut() -> u64, S: FnMut(u64) {
    let (_, mut settings) = match read_settings_and_bytes(path) {
        Ok(value) => value,
        Err(_) => { let _ = writeln!(err, "The settings file could not be read or is invalid; enforcement did not start."); return 1; }
    };
    if let Err(problem) = choose_zone(None, zone_name().as_deref()) {
        let _ = writeln!(err, "{problem}"); return 1;
    }
    let mut last_problem = String::new();
    let mut previous_start: Option<u64> = None;
    loop {
        let started = monotonic_ms();
        if let Some(previous) = previous_start {
            let interval = started.saturating_sub(previous);
            if interval > 1000 { let _ = writeln!(err, "Enforcement checks were {interval} ms apart; the one-second interval was missed."); }
        }
        previous_start = Some(started);
        match read_settings_and_bytes(path) {
            Ok((_, next)) => { settings = next; last_problem.clear(); }
            Err(problem) if problem != last_problem => {
                let _ = writeln!(err, "Settings reload failed; keeping last valid settings.");
                last_problem = problem;
            }
            Err(_) => {}
        }
        let now = match clock() {
            Ok(now) => now,
            Err(problem) => { let _ = writeln!(err, "{problem}"); return 1; }
        };
        let zone = match choose_zone(None, zone_name().as_deref()) {
            Ok((_, zone)) => zone,
            Err(problem) => { let _ = writeln!(err, "{problem}"); return 1; }
        };
        match apps() {
            Ok(current) => judge_apps(&settings, &current, now, &zone, own_pid, &mut terminate, err),
            Err(problem) => { let _ = writeln!(err, "{problem}"); }
        }
        let elapsed = monotonic_ms().saturating_sub(started);
        if elapsed >= 1000 { let _ = writeln!(err, "Enforcement check took {elapsed} ms; the one-second interval was missed."); }
        // Leave time for scheduling and bookkeeping before the one-second target.
        sleep(900_u64.saturating_sub(elapsed));
    }
}
