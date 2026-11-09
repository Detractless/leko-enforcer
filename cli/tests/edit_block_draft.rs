mod edit_support;
use edit_support::*;
use edit_runs::{command, fixture, NOW};

#[test]
fn edit_previews_coverage_and_saves_nothing() {
    let (_folder, file) = fixture(Some(24));
    let before = std::fs::read(&file).unwrap();
    let run = command(&file, "edit-block", &["--id", "one", "--remove-app", "com.two"], NOW);
    assert_eq!(run.code, 0, "{}", run.err);
    assert!(run.out.contains("Current coverage") && run.out.contains("Proposed coverage") && run.out.contains("must wait") && run.out.contains("Nothing was saved"), "{}", run.out);
    assert_eq!(before, std::fs::read(file).unwrap());
}
