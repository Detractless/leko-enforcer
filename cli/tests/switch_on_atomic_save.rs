//! The save of `leko switch-on` goes through the temporary-file-then-move step (AC-011): when the write,
//! the flush or the move fails, or the folder cannot be written, the settings zip is exactly as it was, no
//! temporary file is left, the answer is exit 1 with one sentence that names the step, and the same
//! command run again with a working disk finishes the job. Only a switch-on that changes the zip saves.

mod delay_runs;
#[path = "../../core/tests/delay_sets/mod.rs"]
mod delay_sets;
mod run_capture;
mod scratch_folder;
#[path = "../../core/tests/zip_writer/mod.rs"]
mod zip_writer;

use delay_runs::{block_of, record, snapshot, switch_on, switch_on_with, waiting_of, write_waiting, write_zip, FailAt, Step};
use delay_sets::off_block;
use leko_cli::session::session_report::shown;
use scratch_folder::Scratch;
use std::path::PathBuf;

const STEPS: [Step; 3] = [Step::Write, Step::Flush, Step::Rename];

fn folder_with_two() -> (Scratch, PathBuf) {
    let folder = Scratch::new("switch-on-atomic");
    let file = write_zip(folder.path(), "s.zip", &[off_block("a"), off_block("b")], &[]);
    write_waiting(&file, &[record("b", "2026-11-02T13:00:00Z", "2026-11-03T13:00:00Z")]);
    (folder, file)
}

#[test]
fn a_failure_at_any_step_leaves_the_settings_zip_as_it_was() {
    for step in STEPS {
        let (folder, file) = folder_with_two();
        let before = snapshot(folder.path());
        let run = switch_on_with(&file, &["--id", "a", "--write"], &mut FailAt::new(step, 1));
        assert_eq!((run.code, run.out.as_str()), (1, ""), "{step:?}: {}", run.err);
        let expected = format!("The change could not be saved: {} ({}). The settings file {} was left as it was.\n", step.told_as(), step.problem(), shown(&file.to_string_lossy()));
        assert_eq!(run.err, expected, "{step:?}: one sentence naming the step");
        assert_eq!(snapshot(folder.path()), before, "{step:?}: the zip, the waiting file and the folder are as they were, no temporary file");
        assert_eq!(block_of(&file, "a")["on"], false, "{step:?}: the block is still off");

        // The disk works again: the same command finishes the job.
        let again = switch_on(&file, &["--id", "a", "--write"]);
        assert_eq!((again.code, again.err.as_str()), (0, ""), "{step:?}, again");
        assert!(again.out.starts_with("Saved:"), "{step:?}, again: {}", again.out);
        assert_eq!(block_of(&file, "a")["on"], true, "{step:?}, again");
        assert_eq!(block_of(&file, "b")["on"], false, "{step:?}, again: only `a` changed");
    }
}

#[test]
fn a_save_moves_exactly_one_file_into_place_and_only_when_something_changes() {
    let (folder, file) = folder_with_two();
    let mut steps = FailAt::recording();
    let run = switch_on_with(&file, &["--id", "a", "--write"], &mut steps);
    assert_eq!((run.code, run.err.as_str()), (0, ""));
    assert_eq!(steps.moved_to, ["s.zip"], "one move, into the settings zip, never into the waiting file");
    assert_eq!(folder.names(), ["s.zip", "s.zip.waiting.json"]);

    // A draft, a block that is on, an unknown block, a parked switch-off and a bad request never reach a
    // step of the save: a disk that fails at the first step changes nothing and does not matter.
    let before = snapshot(folder.path());
    for (flags, code) in [(&["--id", "a"][..], 0), (&["--id", "a", "--write"], 0), (&["--id", "zzz", "--write"], 1), (&["--id", "b", "--write"], 1), (&["--id", "a", "--write", "--write"], 2)] {
        let run = switch_on_with(&file, flags, &mut FailAt::new(Step::Write, 1));
        assert_eq!(run.code, code, "{flags:?}: {}", run.err);
        assert_eq!(snapshot(folder.path()), before, "{flags:?}");
    }
    assert!(waiting_of(&file).is_file());
}

#[cfg(unix)]
#[test]
fn a_folder_nobody_may_write_stops_the_save_before_a_temporary_file_is_made() {
    let (folder, file) = folder_with_two();
    let before = snapshot(folder.path());
    folder.set_mode(0o555);
    if std::fs::File::create(folder.path().join("probe")).is_err() {
        let run = switch_on(&file, &["--id", "a", "--write"]);
        assert_eq!((run.code, run.out.as_str()), (1, ""), "{}", run.err);
        assert!(run.err.starts_with("The change could not be saved: a temporary file could not be made beside it (Permission denied"), "{}", run.err);
        assert!(run.err.ends_with(&format!("The settings file {} was left as it was.\n", shown(&file.to_string_lossy()))) && run.err.lines().count() == 1, "{}", run.err);
        folder.set_mode(0o755);
        assert_eq!(snapshot(folder.path()), before, "nothing came, went or changed");
    }
    folder.set_mode(0o755);
}
