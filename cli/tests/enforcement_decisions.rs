use leko_cli::enforcement::enforcement_loop::judge_apps;
use leko_cli::enforcement::macos_applications::Application;
use leko_core::config_time::zone_time::ZoneTime;
use leko_core::models::specs::{AppCover, BlockSpec, Door, Lock, ReadSet, Rule};

fn set(cover: AppCover, kind: &str) -> ReadSet {
    let mut set = ReadSet::default();
    set.blocks.insert("block".into(), BlockSpec {
        id: "block".into(), on: true, apps: cover, sites: vec![],
        rules: vec![Rule { id: "rule".into(), kind: kind.into(), days: vec![], allowed: vec![], ends_at: None }],
        door: Door::Wall, pass: None, earn: None, lock: Lock::open(),
    });
    set
}
fn app(pid: i32, bundle: Option<&str>) -> Application {
    Application { pid, bundle: bundle.map(str::to_string) }
}

#[test]
fn only_closed_running_apps_are_terminated() {
    let settings = set(AppCover::Named(vec!["com.example.blocked".into()]), "always");
    let apps = [app(1, Some("com.example.blocked")), app(2, Some("com.example.open")), app(3, Some("com.example.blocked"))];
    let mut called = Vec::new();
    judge_apps(&settings, &apps, 1_700_000_000_000, &ZoneTime::new("UTC").unwrap(), 99, |app| { called.push(app.pid); Ok(()) }, &mut Vec::new());
    assert_eq!(called, [1, 3]);
}

#[test]
fn running_apps_are_judged_against_settings() {
    let settings = set(AppCover::Named(vec!["com.example.app".into()]), "always");
    let mut calls = 0;
    judge_apps(&settings, &[app(1, Some("com.example.app"))], 1_700_000_000_000, &ZoneTime::new("UTC").unwrap(), 99, |_| { calls += 1; Ok(()) }, &mut Vec::new());
    assert_eq!(calls, 1);
}

#[test]
fn essential_and_unidentified_apps_are_not_terminated() {
    let settings = set(AppCover::AllExcept(vec![]), "always");
    let apps = [app(1, Some("com.apple.finder")), app(2, Some("com.apple.dock")), app(3, Some("com.apple.loginwindow")), app(4, Some("com.apple.systemuiserver")), app(5, None), app(6, Some("com.example.self")), app(7, Some("com.example.other"))];
    let mut called = Vec::new();
    let mut errors = Vec::new();
    judge_apps(&settings, &apps, 1_700_000_000_000, &ZoneTime::new("UTC").unwrap(), 6, |app| { called.push(app.pid); Ok(()) }, &mut errors);
    assert_eq!(called, [7]);
    assert!(String::from_utf8(errors).unwrap().contains("no bundle ID"));
}

#[test]
fn unsupported_usage_rules_are_reported() {
    let settings = set(AppCover::Named(vec!["com.example.app".into()]), "minutes");
    let mut errors = Vec::new();
    judge_apps(&settings, &[app(1, Some("com.example.app"))], 1_700_000_000_000, &ZoneTime::new("UTC").unwrap(), 99, |_| panic!("usage rule cannot terminate"), &mut errors);
    assert!(String::from_utf8(errors).unwrap().contains("Not judged"));
}

#[test]
fn termination_failure_does_not_stop_other_apps() {
    let settings = set(AppCover::AllExcept(vec![]), "always");
    let mut called = Vec::new();
    let mut errors = Vec::new();
    judge_apps(&settings, &[app(1, Some("com.example.one")), app(2, Some("com.example.two"))], 1_700_000_000_000, &ZoneTime::new("UTC").unwrap(), 99, |app| { called.push(app.pid); if app.pid == 1 { Err("refused".into()) } else { Ok(()) } }, &mut errors);
    assert_eq!(called, [1, 2]);
    assert!(String::from_utf8(errors).unwrap().contains("will retry"));
}

#[cfg(target_os = "macos")]
#[test]
fn appkit_lists_real_running_apps() {
    let apps = leko_cli::enforcement::macos_applications::running().unwrap();
    assert!(apps.iter().any(|app| app.bundle.as_deref() == Some("com.apple.finder")));
}
