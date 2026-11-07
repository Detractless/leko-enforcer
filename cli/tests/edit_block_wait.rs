mod edit_runs;
mod run_capture;
mod scratch_folder;
mod settings_builder;
use edit_runs::{command, fixture, waiting, NOW};

#[test]
fn locked_edits_are_parked_without_changing_settings() {
    let (_folder, file) = fixture(Some(24));
    let before = std::fs::read(&file).unwrap();
    let run = command(&file, "edit-block", &["--id", "one", "--remove-app", "com.two", "--write"], NOW);
    assert_eq!(run.code, 0, "{}", run.err);
    assert!(run.out.contains("Waiting"));
    assert_eq!(before, std::fs::read(&file).unwrap());
    let value: serde_json::Value = serde_json::from_slice(&std::fs::read(waiting(&file)).unwrap()).unwrap();
    assert_eq!(value["format"], 2);
    assert_eq!(value["changes"][0]["operations"][0]["target"], "com.two");
}
