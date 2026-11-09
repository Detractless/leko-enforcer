mod edit_support;
use edit_support::*;
use edit_runs::{command, fixture, NOW};

#[test]
fn status_shows_edits_for_affected_targets() {
    let (_folder, file) = fixture(Some(24));
    let edit = command(&file, "edit-block", &["--id", "one", "--remove-app", "com.two", "--add-site", "example.com", "--write"], NOW);
    assert_eq!(edit.code, 0, "{}", edit.err);
    let status = command(&file, "status", &["--site", "sub.example.com"], NOW);
    assert_eq!(status.code, 0, "{}", status.err);
    assert!(status.out.contains("add-site") && status.out.contains("example.com") && status.out.contains("Due time:"), "{}", status.out);
    let status = command(&file, "status", &["--json", "--app", "com.two"], NOW);
    assert_eq!(status.code, 0, "{}", status.err);
    let value: serde_json::Value = serde_json::from_str(&status.out).unwrap();
    assert_eq!(value["waiting"][0]["kind"], "edit-coverage");
    assert!(value["waiting"][0]["from"].is_string());
    assert!(value["waiting"][0]["operations"].as_array().unwrap().len() == 2);
}
