use std::ffi::OsString;
use std::fs;
use std::path::PathBuf;

fn call(args: &[&str]) -> (i32, String, String) {
    let raw: Vec<OsString> = args.iter().map(OsString::from).collect();
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = leko_cli::run(&raw, &mut out, &mut err);
    (code, String::from_utf8(out).unwrap(), String::from_utf8(err).unwrap())
}

fn temp_dir(label: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("leko-cli-args-{}-{}", std::process::id(), label));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).unwrap();
    path
}

#[test]
fn only_canonical_rule_and_infrastructure_commands_are_accepted() {
    for legacy in ["add-block", "session", "switch-off", "status", "edit-block", "delay"] {
        let (code, out, err) = call(&[legacy]);
        assert_eq!(code, 2, "legacy command {} was accepted", legacy);
        assert!(out.is_empty());
        assert!(err.contains("legacy command"));
    }
    let (code, _, err) = call(&["nonsense"]);
    assert_eq!(code, 2);
    assert!(err.contains("Unknown command"));

    let (code, out, err) = call(&["settings", "--json"]);
    assert_eq!(code, 0, "{}", err);
    assert!(out.contains("shadow_mode"));

    let (code, _, err) = call(&["create-rule", "--open-hours", "09:00"]);
    assert_eq!(code, 2);
    assert!(err.contains("legacy flag"));

    let (code, _, _) = call(&["create-rule"]);
    assert_eq!(code, 2);
}

#[test]
fn wrong_or_missing_arguments_print_usage_to_stderr_and_exit_non_zero() {
    let (code, out, err) = call(&[]);
    assert_eq!(code, 2);
    assert!(out.is_empty());
    assert!(err.contains("Usage:"));

    let (code, out, err) = call(&["create-rule", "--name"]);
    assert_eq!(code, 2);
    assert!(out.is_empty());
    assert!(err.contains("needs a value"));

    let (code, out, err) = call(&["create-rule", "--store", "--name", "x"]);
    assert_eq!(code, 2);
    assert!(out.is_empty());
    assert!(err.contains("needs a value"));

    let (code, out, err) = call(&["show-rule", "--id", "missing", "--store", "does-not-exist.json"]);
    assert_eq!(code, 1);
    assert!(out.is_empty());
    assert!(!err.is_empty());
}

#[test]
fn repeated_and_empty_input_commands_are_safe_and_idempotent() {
    let dir = temp_dir("idempotent");
    let store = dir.join("store.json");
    let path = store.to_str().unwrap();
    let target = r#"{"kind":"specific_apps","apps":["notes"]}"#;
    let command = [
        "create-rule", "--store", path, "--id", "r1", "--name", "Study",
        "--target", target, "--penalty", r#""force_close_penalty""#,
        "--at", "2026-01-01T00:00:00Z",
    ];

    let (first, out, err) = call(&command);
    assert_eq!(first, 0, "{} {}", out, err);
    let (second, _, err) = call(&command);
    assert_eq!(second, 1);
    assert!(err.contains("already exists"));

    let (code, out, _) = call(&["list-rules", "--store", path]);
    assert_eq!(code, 0);
    assert!(out.contains("1"));

    let empty = dir.join("empty.json");
    let (code, out, _) = call(&["list-rules", "--store", empty.to_str().unwrap()]);
    assert_eq!(code, 0);
    assert!(out.contains("No rules"));

    fs::remove_dir_all(dir).unwrap();
}
