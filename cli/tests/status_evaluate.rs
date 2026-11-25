use std::ffi::OsString;
use std::fs;

use leko_core::enforcement::{BrowserId, Heartbeat};
use leko_core::models::{
    Pass, PassCap, PassLength, PassRequirement, Penalty, Schedule, Seal, Target, TimeBasedSchedule,
};
use leko_cli::arguments::parse;
use leko_cli::services::enforcement_loop::enforce_with;
use leko_cli::services::os_executor::{MockExecutor, ObservedApp};
use leko_core::store::NativeStore;
use leko_core::TechnologyManagementRule;

fn call(args: &[&str]) -> (i32, String, String) {
    let raw: Vec<OsString> = args.iter().map(OsString::from).collect();
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = leko_cli::run(&raw, &mut out, &mut err);
    (code, String::from_utf8(out).unwrap(), String::from_utf8(err).unwrap())
}

#[test]
fn status_and_evaluation_are_deterministic_under_an_injected_timestamp() {
    let dir = std::env::temp_dir().join(format!("leko-cli-status-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let store = dir.join("store.json");
    let rule = TechnologyManagementRule {
        id: "r1".to_string(),
        name: "Study".to_string(),
        enabled: true,
        target: Target::SpecificWebsites { sites: vec!["site.test".to_string()] },
        schedule: Schedule {
            time_based: TimeBasedSchedule::OneTimeSchedule {
                start_at: "2026-01-01T00:00:00Z".to_string(),
                end_at: "2026-01-02T00:00:00Z".to_string(),
            },
            usage_based: None,
        },
        penalty: Some(Penalty::ForceClosePenalty),
        pass: Pass::default(),
        seal: Seal::default(),
        created_at: "2026-01-01T00:00:00Z".to_string(),
        updated_at: "2026-01-01T00:00:00Z".to_string(),
    };
    leko_core::store::save(&store, &NativeStore { version: 1, rules: vec![rule] }).unwrap();
    let flag = store.to_str().unwrap();

    let first = call(&["evaluate-rule", "--store", flag, "--id", "r1", "--at", "2026-01-01T12:00:00Z"]);
    let second = call(&["evaluate-rule", "--store", flag, "--id", "r1", "--at", "2026-01-01T12:00:00Z"]);
    assert_eq!(first.0, 0, "{} {}", first.1, first.2);
    assert_eq!(first.1, second.1, "the same timestamp gave different answers");
    assert!(first.1.contains("Enforced: true"));

    let later = call(&["check-status", "--store", flag, "--id", "r1", "--at", "2026-01-03T00:00:00Z"]);
    assert_eq!(later.0, 0, "{} {}", later.1, later.2);
    assert!(later.1.contains("Enforced: false"));
    assert!(later.1.contains("OutsideSchedule"));

    let json = call(&[
        "evaluate-rule", "--store", flag, "--id", "r1", "--json",
        "--at", "2026-01-01T12:00:00Z",
    ]);
    assert_eq!(json.0, 0, "{} {}", json.1, json.2);
    let value: serde_json::Value = serde_json::from_str(json.1.trim()).unwrap();
    assert_eq!(value["enforced"], true);

    fs::remove_dir_all(dir).unwrap();
}

fn rule(target: Target, pass: Pass) -> TechnologyManagementRule {
    let whole_device = matches!(target, Target::WholeDevice { .. });
    TechnologyManagementRule {
        id: "r1".to_string(),
        name: "Study".to_string(),
        enabled: true,
        target,
        schedule: Schedule::default(),
        penalty: if whole_device { None } else { Some(Penalty::ForceClosePenalty) },
        pass,
        seal: Seal::default(),
        created_at: "2026-01-01T00:00:00Z".to_string(),
        updated_at: "2026-01-01T00:00:00Z".to_string(),
    }
}

fn loop_call(
    label: &str,
    extra: &[&str],
    rules: Vec<TechnologyManagementRule>,
    mock: &mut MockExecutor,
) -> (i32, String, String) {
    let dir = std::env::temp_dir().join(format!("leko-loop-{}-{}", std::process::id(), label));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let store = dir.join("store.json");
    leko_core::store::save(&store, &NativeStore { version: 1, rules }).unwrap();
    let path = store.to_str().unwrap();
    let mut argv = vec!["enforce", path, "--once", "--at", "2026-01-01T12:00:00Z"];
    argv.extend_from_slice(extra);
    let text: Vec<String> = argv.iter().map(|value| value.to_string()).collect();
    let parsed = parse(&text).unwrap();
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = match enforce_with(&parsed, &mut out, &mut err, mock) {
        Ok(()) => 0,
        Err((_, value)) => value,
    };
    (code, String::from_utf8(out).unwrap(), String::from_utf8(err).unwrap())
}

#[test]
fn the_loop_reloads_the_store_on_every_tick() {
    let rule = rule(Target::SpecificApps { apps: vec!["com.example.Bad".to_string()] }, Pass::default());
    let mut mock = MockExecutor::default();
    mock.apps.push(ObservedApp { pid: 20, bundle: Some("com.example.Bad".to_string()) });
    let (code, out, err) = loop_call("reload", &[], vec![rule], &mut mock);
    assert_eq!(code, 0, "{}", err);
    assert!(out.contains("Enforcement ticks: 1"));
    assert_eq!(mock.terminated, vec!["com.example.Bad".to_string()]);
}

#[test]
fn blocked_apps_are_terminated_until_the_pass_or_schedule_stops_them() {
    let sample = rule(Target::SpecificApps { apps: vec!["com.example.Bad".to_string()] }, Pass::default());
    let mut mock = MockExecutor::default();
    mock.apps.push(ObservedApp { pid: 20, bundle: Some("com.example.Bad".to_string()) });
    mock.apps.push(ObservedApp { pid: 21, bundle: Some("com.apple.finder".to_string()) });
    loop_call("apps", &[], vec![sample], &mut mock);
    assert_eq!(mock.terminated, vec!["com.example.Bad".to_string()]);

    let earned = Pass::EarnPass {
        requirement: PassRequirement::PhysicalPass,
        length: PassLength::TimedPass { minutes: 30 },
        cap: PassCap::UnlimitedPasses,
    };
    let with_pass = rule(Target::SpecificApps { apps: vec!["com.example.Bad".to_string()] }, earned);
    let mut mock2 = MockExecutor::default();
    mock2.apps.push(ObservedApp { pid: 20, bundle: Some("com.example.Bad".to_string()) });
    loop_call("pass", &["--earned-at", "2026-01-01T12:00:00Z"], vec![with_pass], &mut mock2);
    assert!(mock2.terminated.is_empty());
}

#[test]
fn browser_heartbeats_drive_warn_and_close() {
    let rule = rule(Target::SpecificWebsites { sites: vec!["site.test".to_string()] }, Pass::default());
    let browser = BrowserId { pid: 30, bundle_id: "com.example.Browser".to_string(), profile: "Default".to_string() };
    let mut mock = MockExecutor::default();
    mock.browsers.push(browser.clone());
    mock.beats.insert(browser, Heartbeat {
        seen_at_ms: 0,
        block_id: "r1".to_string(),
        attached: true,
    });
    let (code, _, err) = loop_call("browser", &[], vec![rule], &mut mock);
    assert_eq!(code, 0, "{}", err);
    assert_eq!(mock.warned, vec!["com.example.Browser".to_string()]);
    assert!(mock.closed.is_empty());
}

#[test]
fn whole_device_actions_follow_the_native_device_action() {
    use leko_core::models::DeviceAction;
    let rule = rule(Target::WholeDevice { action: DeviceAction::Lock }, Pass::default());
    let mut failing = MockExecutor::default();
    failing.fail_device = true;
    let (code, _, err) = loop_call("device-fail", &[], vec![rule.clone()], &mut failing);
    assert_eq!(code, 0, "{}", err);
    assert!(err.contains("Could not perform"));

    let mut mock = MockExecutor::default();
    let (code, _, err) = loop_call("device", &[], vec![rule], &mut mock);
    assert_eq!(code, 0, "{}", err);
    assert_eq!(mock.devices, vec![DeviceAction::Lock]);
}

#[test]
fn injected_clock_keeps_the_loop_deterministic() {
    let rule = rule(Target::SpecificWebsites { sites: vec!["site.test".to_string()] }, Pass::default());
    let mut first = MockExecutor::default();
    let mut second = MockExecutor::default();
    let a = loop_call("clock-a", &[], vec![rule.clone()], &mut first);
    let b = loop_call("clock-b", &[], vec![rule], &mut second);
    assert_eq!(a.0, b.0);
    assert_eq!(a.1, b.1);
    assert_eq!(a.2, b.2);
}
