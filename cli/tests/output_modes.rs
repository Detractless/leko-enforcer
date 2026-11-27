use std::ffi::OsString;
use std::fs;

fn call(args: &[&str]) -> (i32, String, String) {
    let raw: Vec<OsString> = args.iter().map(OsString::from).collect();
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = leko_cli::run(&raw, &mut out, &mut err);
    (code, String::from_utf8(out).unwrap(), String::from_utf8(err).unwrap())
}

#[test]
fn json_output_is_a_stable_canonical_object_for_milestone_three() {
    let dir = std::env::temp_dir().join(format!("leko-cli-json-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let store = dir.join("store.json");
    let flag = store.to_str().unwrap();
    let name = "Focus \"深度\"";

    let created = call(&[
        "create-rule", "--store", flag, "--id", "r1", "--name", name,
        "--target", r#"{"kind":"specific_apps","apps":["notes"]}"#,
        "--penalty", r#""force_close_penalty""#,
        "--at", "2026-01-01T00:00:00Z",
    ]);
    assert_eq!(created.0, 0, "{} {}", created.1, created.2);

    let show = call(&["show-rule", "--store", flag, "--id", "r1", "--json"]);
    assert_eq!(show.0, 0, "{} {}", show.1, show.2);
    let value: serde_json::Value = serde_json::from_str(show.1.trim()).unwrap();

    for key in [
        "id", "name", "enabled", "target", "schedule", "penalty", "pass", "seal",
        "created_at", "updated_at",
    ] {
        assert!(value.get(key).is_some(), "missing canonical key {}", key);
    }
    assert!(value["target"].is_object());
    assert!(value["schedule"].is_object());
    assert!(value["pass"].is_object());
    assert!(value["seal"].is_object());
    assert_eq!(value["name"], name);
    assert_eq!(value["target"]["kind"], "specific_apps");
    assert_eq!(value["pass"]["kind"], "no_pass");
    assert_eq!(value["seal"]["kind"], "no_seal");
    assert!(!show.1.contains("Usage:"), "prose leaked into the JSON stream");

    let listed = call(&["list-rules", "--store", flag, "--json"]);
    assert_eq!(listed.0, 0, "{} {}", listed.1, listed.2);
    let list: serde_json::Value = serde_json::from_str(listed.1.trim()).unwrap();
    assert_eq!(list["rules"].as_array().unwrap().len(), 1);

    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn status_and_evaluate_report_remaining_usage_budgets() {
    let dir = std::env::temp_dir().join(format!("leko-cli-usage-out-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let store = dir.join("store.json");
    let usage = dir.join("store.usage-state.json");
    let flag = store.to_str().unwrap();

    let created = call(&[
        "create-rule", "--store", flag, "--id", "r1", "--name", "Focus",
        "--target", r#"{"kind":"specific_apps","apps":["com.example.Bad"]}"#,
        "--schedule", r#"{"time_based":{"kind":"always_on_schedule"},"usage_based":{"kind":"time_limit","minutes":10,"period":"daily"}}"#,
        "--penalty", r#""force_close_penalty""#,
        "--at", "2026-01-01T00:00:00Z",
    ]);
    assert_eq!(created.0, 0, "{} {}", created.1, created.2);

    fs::write(&usage, serde_json::to_vec(&serde_json::json!({
        "version": 1,
        "rules": {
            "r1": {
                "rule_id": "r1",
                "period_key": "2026-01-01",
                "active_seconds": 180,
                "sessions": 1,
                "session_open": true,
                "last_active_at": "2026-01-01T12:03:00Z",
                "away_since": null,
                "engagement_seconds": 0,
                "engagement_window_key": "2026-01-01",
                "last_engagement_at": null
            }
        }
    })).unwrap()).unwrap();

    let json_status = call(&["check-status", "--store", flag, "--id", "r1", "--at", "2026-01-01T12:03:00Z", "--json"]);
    assert_eq!(json_status.0, 0, "{}", json_status.2);
    let val: serde_json::Value = serde_json::from_str(json_status.1.trim()).unwrap();
    assert_eq!(val["enforced"], false);
    assert_eq!(val["usage"]["kind"], "Time Limit");
    assert_eq!(val["usage"]["used_seconds"], 180);
    assert_eq!(val["usage"]["remaining_seconds"], 420);

    let human_status = call(&["check-status", "--store", flag, "--id", "r1", "--at", "2026-01-01T12:03:00Z"]);
    assert_eq!(human_status.0, 0, "{}", human_status.2);
    assert!(human_status.1.contains("Usage Limit: Time Limit"));
    assert!(human_status.1.contains("Remaining Seconds: 420"));

    let eval_override = call(&[
        "evaluate-rule", "--store", flag, "--id", "r1",
        "--used-minutes", "10", "--at", "2026-01-01T12:03:00Z", "--json",
    ]);
    assert_eq!(eval_override.0, 0, "{}", eval_override.2);
    let eval_val: serde_json::Value = serde_json::from_str(eval_override.1.trim()).unwrap();
    assert_eq!(eval_val["enforced"], true);
    assert_eq!(eval_val["usage"]["remaining_seconds"], 0);
    assert!(!json_status.1.contains("Usage:"));

    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn time_limit_counts_visible_window_and_pauses_when_hidden_while_session_ignores_visible() {
    use leko_cli::arguments::parse;
    use leko_cli::services::enforcement_loop::enforce_with;
    use leko_cli::services::os_executor::{ForegroundActivity, MockExecutor, ObservedApp};
    use leko_core::models::{Pass, Penalty, Period, Schedule, Seal, Target, TimeBasedSchedule, UsageLimit};
    use leko_core::store::{save, NativeStore};
    use leko_core::TechnologyManagementRule;

    let dir = std::env::temp_dir().join(format!("leko-cli-visible-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let store = dir.join("store.json");
    let usage = dir.join("usage.json");
    let t_rule = TechnologyManagementRule {
        id: "t1".into(), name: "VisibleLimit".into(), enabled: true,
        target: Target::SpecificApps { apps: vec!["com.example.Bad".into()] },
        schedule: Schedule {
            time_based: TimeBasedSchedule::AlwaysOnSchedule,
            usage_based: Some(UsageLimit::TimeLimit { minutes: 1, period: Period::Daily }),
        },
        penalty: Some(Penalty::ForceClosePenalty), pass: Pass::NoPass, seal: Seal::NoSeal,
        created_at: "2026-01-01T00:00:00Z".into(), updated_at: "2026-01-01T00:00:00Z".into(),
    };
    save(&store, &NativeStore { version: 1, rules: vec![t_rule] }).unwrap();

    let mut mock = MockExecutor::default();
    mock.apps.push(ObservedApp { pid: 7, bundle: Some("com.example.Bad".into()) });
    mock.foreground = Some(ForegroundActivity {
        bundle_id: Some("com.apple.Notes".into()),
        visible_apps: vec!["com.apple.Notes".into(), "com.example.Bad".into()],
        ..Default::default()
    });
    for at in ["2026-01-01T12:00:00Z", "2026-01-01T12:00:30Z"] {
        let argv = ["enforce", store.to_str().unwrap(), "--once", "--usage-state", usage.to_str().unwrap(), "--at", at]
            .iter().map(|s| s.to_string()).collect::<Vec<String>>();
        enforce_with(&parse(&argv).unwrap(), &mut Vec::new(), &mut Vec::new(), &mut mock).unwrap();
    }
    assert_eq!(leko_core::services::load_usage_state(&usage).unwrap().rules["t1"].active_seconds, 30);
    assert!(mock.terminated.is_empty());

    mock.foreground = Some(ForegroundActivity {
        bundle_id: Some("com.apple.Notes".into()),
        visible_apps: vec!["com.apple.Notes".into()],
        ..Default::default()
    });
    for at in ["2026-01-01T12:00:40Z", "2026-01-01T12:01:20Z"] {
        let argv = ["enforce", store.to_str().unwrap(), "--once", "--usage-state", usage.to_str().unwrap(), "--at", at]
            .iter().map(|s| s.to_string()).collect::<Vec<String>>();
        enforce_with(&parse(&argv).unwrap(), &mut Vec::new(), &mut Vec::new(), &mut mock).unwrap();
    }
    assert_eq!(leko_core::services::load_usage_state(&usage).unwrap().rules["t1"].active_seconds, 30);
    assert!(mock.terminated.is_empty());

    mock.foreground = Some(ForegroundActivity {
        bundle_id: Some("com.apple.Notes".into()),
        visible_apps: vec!["com.apple.Notes".into(), "com.example.Bad".into()],
        ..Default::default()
    });
    for at in ["2026-01-01T12:02:00Z", "2026-01-01T12:02:35Z"] {
        let argv = ["enforce", store.to_str().unwrap(), "--once", "--usage-state", usage.to_str().unwrap(), "--at", at]
            .iter().map(|s| s.to_string()).collect::<Vec<String>>();
        enforce_with(&parse(&argv).unwrap(), &mut Vec::new(), &mut Vec::new(), &mut mock).unwrap();
    }
    assert_eq!(leko_core::services::load_usage_state(&usage).unwrap().rules["t1"].active_seconds, 65);
    assert_eq!(mock.terminated, vec!["com.example.Bad".to_string()]);

    let s_rule = TechnologyManagementRule {
        id: "s1".into(), name: "Session".into(), enabled: true,
        target: Target::SpecificApps { apps: vec!["com.example.Bad".into()] },
        schedule: Schedule {
            time_based: TimeBasedSchedule::AlwaysOnSchedule,
            usage_based: Some(UsageLimit::SessionLimit { max_sessions: 1, period: Period::Daily }),
        },
        penalty: Some(Penalty::ForceClosePenalty), pass: Pass::NoPass, seal: Seal::NoSeal,
        created_at: "2026-01-01T00:00:00Z".into(), updated_at: "2026-01-01T00:00:00Z".into(),
    };
    let s_usage = dir.join("s_usage.json");
    save(&store, &NativeStore { version: 1, rules: vec![s_rule] }).unwrap();
    mock.terminated.clear();
    let argv = ["enforce", store.to_str().unwrap(), "--once", "--usage-state", s_usage.to_str().unwrap(), "--at", "2026-01-01T12:03:00Z"]
        .iter().map(|s| s.to_string()).collect::<Vec<String>>();
    enforce_with(&parse(&argv).unwrap(), &mut Vec::new(), &mut Vec::new(), &mut mock).unwrap();
    assert_eq!(leko_core::services::load_usage_state(&s_usage).unwrap().rules["s1"].sessions, 0);
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn countdowns_command_sorts_by_remaining_and_urgency_in_json_and_title_case() {
    use leko_core::models::{
        Pass, PassCap, PassLength, PassRequirement, Penalty, Period, QueuedEdit, Schedule, Seal,
        Target, TimeBasedSchedule, UsageLimit,
    };
    use leko_core::services::{save_pass_state, save_usage_state, PassStateFile, RuleUsage, StoredPass, UsageStateFile};
    use leko_core::store::{save, NativeStore};
    use leko_core::TechnologyManagementRule;

    let dir = std::env::temp_dir().join(format!("leko-cli-cd-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let store = dir.join("store.json");
    let usage_path = dir.join("store.usage-state.json");
    let pass_path = dir.join("store.pass-state.json");
    let before_rules = vec![
        TechnologyManagementRule {
            id: "r_seal".into(), name: "DelayedRule".into(), enabled: true,
            target: Target::SpecificApps { apps: vec!["com.example.A".into()] },
            schedule: Schedule::default(), penalty: Some(Penalty::ForceClosePenalty),
            pass: Pass::NoPass,
            seal: Seal::QueuedDelaySeal {
                delay_seconds: 300,
                pending_edits: vec![QueuedEdit {
                    key: "enabled".into(), value: "false".into(),
                    requested_at: "2026-01-01T11:57:00Z".into(), cancelled: false,
                }],
            },
            created_at: "2026-01-01T00:00:00Z".into(), updated_at: "2026-01-01T00:00:00Z".into(),
        },
        TechnologyManagementRule {
            id: "r_pass".into(), name: "PassRule".into(), enabled: true,
            target: Target::SpecificApps { apps: vec!["com.example.B".into()] },
            schedule: Schedule::default(), penalty: Some(Penalty::ForceClosePenalty),
            pass: Pass::EarnPass {
                requirement: PassRequirement::PhysicalPass,
                length: PassLength::TimedPass { minutes: 5 },
                cap: PassCap::UnlimitedPasses,
            },
            seal: Seal::NoSeal,
            created_at: "2026-01-01T00:00:00Z".into(), updated_at: "2026-01-01T00:00:00Z".into(),
        },
        TechnologyManagementRule {
            id: "r_limit".into(), name: "LimitRule".into(), enabled: true,
            target: Target::SpecificApps { apps: vec!["com.example.C".into()] },
            schedule: Schedule {
                time_based: TimeBasedSchedule::AlwaysOnSchedule,
                usage_based: Some(UsageLimit::TimeLimit { minutes: 5, period: Period::Daily }),
            },
            penalty: Some(Penalty::ForceClosePenalty), pass: Pass::NoPass, seal: Seal::NoSeal,
            created_at: "2026-01-01T00:00:00Z".into(), updated_at: "2026-01-01T00:00:00Z".into(),
        },
    ];
    save(&store, &NativeStore { version: 1, rules: before_rules }).unwrap();
    let store_bytes = fs::read(&store).unwrap();
    let mut u = UsageStateFile::default();
    u.rules.insert("r_limit".into(), RuleUsage {
        rule_id: "r_limit".into(), period_key: "2026-01-01".into(), active_seconds: 180, ..Default::default()
    });
    save_usage_state(&usage_path, &u).unwrap();
    let mut p = PassStateFile::default();
    p.passes.insert("r_pass".into(), StoredPass {
        rule_id: "r_pass".into(), requirement: "PhysicalPass".into(),
        earned_at: "2026-01-01T11:57:00Z".into(), window_key: "2026-01-01".into(), used_in_window: 1,
    });
    save_pass_state(&pass_path, &p).unwrap();

    let (code, out, err) = call(&["countdowns", "--store", store.to_str().unwrap(), "--at", "2026-01-01T12:00:00Z", "--json"]);
    assert_eq!(code, 0, "{}", err);
    let val: serde_json::Value = serde_json::from_str(out.trim()).unwrap();
    let kinds: Vec<&str> = val["countdowns"].as_array().unwrap().iter().map(|i| i["kind"].as_str().unwrap()).collect();
    assert_eq!(kinds, vec!["Time Limit", "Timed Pass", "Queued Delay Seal"]);
    assert_eq!(val["soonest"]["remaining_seconds"], 120);

    let (hcode, hout, herr) = call(&["countdowns", "--store", store.to_str().unwrap(), "--at", "2026-01-01T12:00:00Z"]);
    assert_eq!(hcode, 0, "{}", herr);
    assert!(hout.contains("Countdowns: 3"));
    assert!(hout.contains("Type: Time Limit"));
    assert!(hout.contains("Remaining Seconds: 120"));
    assert_eq!(fs::read(&store).unwrap(), store_bytes);
    fs::remove_dir_all(dir).unwrap();
}
