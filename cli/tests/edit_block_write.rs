mod edit_support;
use edit_support::*;
use edit_runs::{block, command, fixture, NOW};
use serde_json::json;

#[test]
fn immediate_edits_are_validated_saved_atomically_and_repeat_safely() {
    let (_folder, file) = fixture(Some(24));
    let before = block(&file);
    let args = ["--id", "one", "--add-app", "com.three", "--write"];
    let run = command(&file, "edit-block", &args, NOW);
    assert_eq!(run.code, 0, "{}", run.err);
    let mut expected = before; expected["covers"]["apps"] = json!(["com.one", "com.two", "com.three"]);
    assert_eq!(block(&file), expected);
    let bytes = std::fs::read(&file).unwrap();
    let repeat = command(&file, "edit-block", &args, NOW);
    assert_eq!(repeat.code, 0, "{}", repeat.err);
    assert_eq!(bytes, std::fs::read(file).unwrap());
}
