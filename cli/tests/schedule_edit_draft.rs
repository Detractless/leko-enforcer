mod edit_support;
use edit_support::*;
use edit_runs::{command, NOW};


#[test]
fn schedule_preview_shows_old_new_and_wait_without_writing() {
    let (_folder, file) = schedule_fixture(Some(24));
    let before_bytes = std::fs::read(&file).unwrap();

    // 1. Preview tightening (10:00-16:00): omitted days stay as they were, can land now, writes nothing
    let run = command(&file, "edit-schedule", &["--id", "one", "--rule", "h", "--open-hours", "10:00-16:00"], NOW);
    assert_eq!(run.code, 0, "{}", run.err);
    assert!(run.out.contains("Current rule:"), "{}", run.out);
    assert!(run.out.contains("Proposed rule:"), "{}", run.out);
    assert!(run.out.contains("The change can land now."), "{}", run.out);
    assert!(run.out.contains("Nothing was saved."), "{}", run.out);
    assert_eq!(before_bytes, std::fs::read(&file).unwrap(), "file must not change on preview");

    // 2. Preview loosening (08:00-18:00): must wait, writes nothing
    let run_wait = command(&file, "edit-schedule", &["--id", "one", "--rule", "h", "--open-hours", "08:00-18:00"], NOW);
    assert_eq!(run_wait.code, 0, "{}", run_wait.err);
    assert!(run_wait.out.contains("must wait until"), "{}", run_wait.out);
    assert!(run_wait.out.contains("Nothing was saved."), "{}", run_wait.out);
    assert_eq!(before_bytes, std::fs::read(&file).unwrap(), "file must not change on preview");

    // 3. Preview editing days only (--days all)
    let run_days = command(&file, "edit-schedule", &["--id", "one", "--rule", "h", "--days", "all"], NOW);
    assert_eq!(run_days.code, 0, "{}", run_days.err);
    assert!(run_days.out.contains("Current rule:"), "{}", run_days.out);
    assert!(run_days.out.contains("Proposed rule:"), "{}", run_days.out);
    assert_eq!(before_bytes, std::fs::read(&file).unwrap(), "file must not change on preview");
}
