mod edit_runs;
mod run_capture;
mod scratch_folder;
mod settings_builder;
use edit_runs::{command, fixture, waiting, NOW};

#[test]
fn waiting_changes_on_one_block_never_conflict() {
    let (_folder, file) = fixture(Some(24));
    let initial = std::fs::read(&file).unwrap();
    assert_eq!(command(&file, "edit-block", &["--id", "one", "--remove-app", "com.two", "--write"], NOW).code, 0);
    let queue = std::fs::read(waiting(&file)).unwrap();
    let conflict = command(&file, "switch-off", &["--id", "one", "--write"], NOW);
    assert_eq!(conflict.code, 1);
    assert!(conflict.err.contains("cancel") && conflict.err.contains("settle"));
    assert_eq!(queue, std::fs::read(waiting(&file)).unwrap());
    assert_eq!(command(&file, "cancel", &["--id", "one", "--write"], NOW).code, 0);
    assert_eq!(std::fs::read(&file).unwrap(), initial);
    assert_eq!(command(&file, "switch-off", &["--id", "one", "--write"], NOW).code, 0);
    let conflict = command(&file, "edit-block", &["--id", "one", "--add-app", "com.three", "--write"], NOW);
    assert_eq!(conflict.code, 1);
    assert!(conflict.err.contains("cancel") && conflict.err.contains("settle"));
}
