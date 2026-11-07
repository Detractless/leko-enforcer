mod edit_runs;
mod run_capture;
mod scratch_folder;
mod settings_builder;
use edit_runs::{block, fixture, waiting};
use std::path::Path;
use std::process::{Command, Output};

fn run(file: &Path, command: &str, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_leko")).env("TZ", "UTC").arg(command).arg(file).args(args).output().unwrap()
}

#[test]
fn the_real_program_edits_waits_and_settles() {
    let (_folder, file) = fixture(Some(1));
    let original = std::fs::read(&file).unwrap();
    let draft = run(&file, "edit-block", &["--id", "one", "--add-app", "com.three"]);
    assert!(draft.status.success(), "{}", String::from_utf8_lossy(&draft.stderr));
    assert_eq!(std::fs::read(&file).unwrap(), original);
    let saved = run(&file, "edit-block", &["--id", "one", "--add-app", "com.three", "--write"]);
    assert!(saved.status.success(), "{}", String::from_utf8_lossy(&saved.stderr));
    assert!(block(&file)["covers"]["apps"].as_array().unwrap().iter().any(|x| x == "com.three"));
    let edited = std::fs::read(&file).unwrap();
    let parked = run(&file, "edit-block", &["--id", "one", "--remove-app", "com.two", "--write"]);
    assert!(parked.status.success(), "{}", String::from_utf8_lossy(&parked.stderr));
    assert_eq!(std::fs::read(&file).unwrap(), edited);
    let status = run(&file, "status", &["--app", "com.two"]);
    assert!(status.status.success() && String::from_utf8_lossy(&status.stdout).contains("remove-app"));
    let cancelled = run(&file, "cancel", &["--id", "one", "--write"]);
    assert!(cancelled.status.success(), "{}", String::from_utf8_lossy(&cancelled.stderr));
    let queue: serde_json::Value = serde_json::from_slice(&std::fs::read(waiting(&file)).unwrap()).unwrap();
    assert!(queue["changes"].as_array().unwrap().is_empty());
    assert_eq!(std::fs::read(&file).unwrap(), edited);
}
