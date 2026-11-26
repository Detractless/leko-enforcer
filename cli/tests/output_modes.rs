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
