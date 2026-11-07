mod scratch_folder;
mod settings_builder;
use leko_cli::enforcement::enforcement_loop::run;
use leko_cli::enforcement::macos_applications::Application;
use scratch_folder::Scratch;
use serde_json::json;
use settings_builder::{always, block, write_settings};
use std::cell::Cell;

fn exercise(change_file: bool) -> (usize, String) {
    let scratch = Scratch::new("enforcement-loop");
    let file = write_settings(scratch.path(), "settings.zip", &[block("one", json!({"apps": ["com.example.app"]}), json!([always("a", &[])]))]);
    let ticks = Cell::new(0);
    let calls = Cell::new(0);
    let mut err = Vec::new();
    let code = run(file.to_str().unwrap(), || {
        let n = ticks.get(); ticks.set(n + 1);
        if n < 3 { Ok(1_700_000_000_000 + n * 1000) } else { Err("test ended".into()) }
    }, || Some("UTC".into()), || Ok(vec![Application { pid: 42, bundle: Some("com.example.app".into()) }]),
    |_| { calls.set(calls.get() + 1); Err("refused".into()) }, 99, || 0, |_| {
        if change_file { std::fs::write(&file, b"broken").unwrap(); }
    }, &mut err);
    assert_eq!(code, 1);
    (calls.get(), String::from_utf8(err).unwrap())
}

#[test]
fn each_tick_uses_the_current_time_and_apps() {
    let scratch = Scratch::new("enforcement-boundary");
    let file = write_settings(scratch.path(), "settings.zip", &[block("one", json!({"apps": ["com.example.app"]}), json!([settings_builder::session("a", "2023-11-14T22:13:22Z")]))]);
    let ticks = Cell::new(0);
    let calls = Cell::new(0);
    let mut err = Vec::new();
    run(file.to_str().unwrap(), || { let n = ticks.get(); ticks.set(n + 1); if n < 4 { Ok(1_700_000_000_000 + n * 1000) } else { Err("done".into()) } }, || Some("UTC".into()),
        || if ticks.get() == 1 { Ok(vec![]) } else { Ok(vec![Application { pid: 42, bundle: Some("com.example.app".into()) }]) },
        |_| { calls.set(calls.get() + 1); Ok(()) }, 99, || 0, |_| {}, &mut err);
    assert_eq!(calls.get(), 1, "a later launch closes before the session end, then opens at its boundary");
}

#[test]
fn invalid_reload_keeps_last_valid_settings() {
    let (calls, errors) = exercise(true);
    assert_eq!(calls, 3);
    assert!(errors.contains("keeping last valid settings"));
}

#[test]
fn termination_failure_is_reported_and_retried() {
    let (calls, errors) = exercise(false);
    assert_eq!(calls, 3);
    assert!(errors.contains("refused; will retry"));
}

#[test]
fn ticks_allow_for_processing_time() {
    let scratch = Scratch::new("enforcement-cadence");
    let file = write_settings(scratch.path(), "settings.zip", &[]);
    let starts = std::cell::RefCell::new(Vec::new());
    let count = Cell::new(0);
    let origin = std::time::Instant::now();
    let mut err = Vec::new();
    run(file.to_str().unwrap(), || {
        let n = count.get(); count.set(n + 1);
        if n == 2 { Err("done".into()) } else { Ok(1_700_000_000_000) }
    }, || Some("UTC".into()), || {
        starts.borrow_mut().push(std::time::Instant::now());
        std::thread::sleep(std::time::Duration::from_millis(200));
        Ok(vec![])
    }, |_| Ok(()), 99, || origin.elapsed().as_millis() as u64, |ms| std::thread::sleep(std::time::Duration::from_millis(ms)), &mut err);
    let starts = starts.into_inner();
    let interval = starts[1].duration_since(starts[0]);
    println!("Measured check interval: {interval:?}");
    assert!(interval < std::time::Duration::from_millis(1000), "tick interval was {interval:?}");
}

#[test]
fn reevaluates_after_clock_jumps_and_a_delayed_wakeup() {
    let scratch = Scratch::new("enforcement-clock-jumps");
    let file = write_settings(scratch.path(), "settings.zip", &[block("one", json!({"apps": ["com.example.app"]}), json!([settings_builder::session("a", "2023-11-14T22:13:22Z")]))]);
    let tick = Cell::new(0);
    let calls = Cell::new(0);
    let monotonic = Cell::new(0_u64);
    let mut errors = Vec::new();
    run(file.to_str().unwrap(), || {
        let n = tick.get(); tick.set(n + 1);
        match n { 0 | 2 => Ok(1_700_000_000_000), 1 => Ok(1_700_003_600_000), _ => Err("done".into()) }
    }, || Some("UTC".into()), || Ok(vec![Application { pid: 42, bundle: Some("com.example.app".into()) }]),
    |_| { calls.set(calls.get() + 1); Ok(()) }, 99, || monotonic.get(), |ms| monotonic.set(monotonic.get() + ms + 60_000), &mut errors);
    assert_eq!(calls.get(), 2, "the forward jump opens the app and the backward jump closes it again");
    assert!(String::from_utf8(errors).unwrap().contains("checks were 60900 ms apart"));
}

#[test]
fn a_slow_check_is_reported_and_retried_without_extra_delay() {
    let scratch = Scratch::new("enforcement-overrun");
    let file = write_settings(scratch.path(), "settings.zip", &[]);
    let time = Cell::new(0_u64);
    let ticks = Cell::new(0);
    let delays = std::cell::RefCell::new(Vec::new());
    let mut errors = Vec::new();
    run(file.to_str().unwrap(), || {
        let n = ticks.get(); ticks.set(n + 1);
        if n == 0 { Ok(1_700_000_000_000) } else { Err("done".into()) }
    }, || Some("UTC".into()), || { time.set(time.get() + 1200); Ok(vec![]) }, |_| Ok(()), 99,
    || time.get(), |ms| delays.borrow_mut().push(ms), &mut errors);
    assert_eq!(*delays.borrow(), [0]);
    assert!(String::from_utf8(errors).unwrap().contains("1200 ms; the one-second interval was missed"));
}
