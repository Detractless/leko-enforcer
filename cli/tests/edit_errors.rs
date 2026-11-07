mod edit_runs;
mod run_capture;
mod scratch_folder;
mod settings_builder;
mod delay_runs;
#[path = "../../core/tests/delay_sets/mod.rs"] mod delay_sets;
#[path = "../../core/tests/zip_writer/mod.rs"] mod zip_writer;
use edit_runs::{command, fixture, waiting, NOW};
use leko_cli::edit_block::run_edit_block::run_edit_block_with;
use delay_runs::{FailAt, Step};

#[test]
fn errors_are_reported_and_saves_recover_safely() {
    let (folder, file) = fixture(None);
    let original = std::fs::read(&file).unwrap();
    let bad = command(&file, "edit-block", &["--id", "one", "--add-app"], NOW);
    assert_eq!(bad.code, 2);
    assert!(bad.err.contains("Usage:"));
    std::fs::write(waiting(&file), b"broken").unwrap();
    let bad = command(&file, "edit-block", &["--id", "one", "--add-app", "com.three", "--write"], NOW);
    assert_eq!(bad.code, 1);
    assert_eq!(std::fs::read(waiting(&file)).unwrap(), b"broken");
    assert_eq!(std::fs::read(&file).unwrap(), original);
    std::fs::remove_file(waiting(&file)).unwrap();
    let args = vec![file.to_str().unwrap().to_string(), "--id".into(), "one".into(), "--add-app".into(), "com.three".into(), "--write".into()];
    for step in [Step::Write, Step::Flush, Step::Rename] {
        let mut fail = FailAt::new(step, 1);
        let code = run_edit_block_with(&args, NOW, Some("UTC"), &mut fail, &mut Vec::new(), &mut Vec::new());
        assert_eq!(code, 1);
        assert_eq!(std::fs::read(&file).unwrap(), original);
        assert_eq!(folder.names(), ["settings.zip"]);
    }
}
