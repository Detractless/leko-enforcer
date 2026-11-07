//! Every save goes through the temporary-file-then-move step (AC-010): when the write, the flush or
//! the move fails, or the folder cannot be written, the settings zip and the waiting file are byte for
//! byte as they were, no temporary file is left, the sentence names the step, and the same command
//! run again with a working disk finishes the job.

mod delay_runs;
#[path = "../../core/tests/delay_sets/mod.rs"]
mod delay_sets;
mod run_capture;
mod scratch_folder;
#[path = "../../core/tests/zip_writer/mod.rs"]
mod zip_writer;

use delay_runs::{block_of, record, records_of, settle, settle_with, snapshot, switch_off, switch_off_with, waiting_of, write_waiting, write_zip, FailAt, Step, NOW};
use delay_sets::{locked_block, wait};
use leko_cli::session::session_report::shown;
use run_capture::Outcome;
use scratch_folder::Scratch;
use serde_json::Value;
use std::path::{Path, PathBuf};

const STEPS: [Step; 3] = [Step::Write, Step::Flush, Step::Rename];

/// A folder holding a settings zip of an unlocked block `a`, a block `w` that waits 24 hours and a
/// block `c`, beside a waiting file that holds a parked change for `c` when `with_waiting` is set.
fn folder_with(label: &str, with_waiting: bool) -> (Scratch, PathBuf) {
    let folder = Scratch::new(label);
    let file = write_zip(folder.path(), "s.zip", &[locked_block("a", Value::Null), locked_block("w", wait(24)), locked_block("c", wait(24))], &[]);
    if with_waiting {
        write_waiting(&file, &[record("c", "2026-11-02T14:00:00Z", "2026-11-03T14:00:00Z")]);
    }
    (folder, file)
}

/// The failure is told in one line: who could not be saved, at which step and why, and which file
/// was left as it was.
fn assert_told(run: &Outcome, subject: &str, step: Step, left: &Path, what: &str) {
    assert_eq!((run.code, run.out.as_str()), (1, ""), "{what}: {}", run.err);
    let start = format!("{subject} could not be saved: {} ({}). ", step.told_as(), step.problem());
    let kind = if left.to_string_lossy().ends_with(".waiting.json") { "waiting file" } else { "settings file" };
    let end = format!("The {kind} {} was left as it was.\n", shown(&left.to_string_lossy()));
    assert_eq!(run.err, format!("{start}{end}"), "{what}");
}

#[test]
fn a_failure_at_any_step_leaves_both_files_as_they_were() {
    // A change that lands now: the zip is the file being saved. The parked change for `c` stays.
    for step in STEPS {
        let (folder, file) = folder_with("atomic-now", true);
        let before = snapshot(folder.path());
        let run = switch_off_with(&file, &["--id", "a", "--write"], NOW, &mut FailAt::new(step, 1));
        assert_told(&run, "The change", step, &file, &format!("landing, {step:?}"));
        assert_eq!(snapshot(folder.path()), before, "landing, {step:?}: both files and the folder as they were, no temporary file");
        // The disk works again: the same command finishes the job.
        let again = switch_off(&file, &["--id", "a", "--write"], NOW);
        assert_eq!((again.code, again.err.as_str()), (0, ""), "landing, {step:?}, again");
        assert_eq!(block_of(&file, "a")["on"], false, "landing, {step:?}, again");
        assert_eq!(records_of(&file).len(), 1, "the parked change for `c` was not touched");
    }

    // A change that must wait: the waiting file is the file being saved, whether it exists or not.
    for with_waiting in [true, false] {
        for step in STEPS {
            let (folder, file) = folder_with("atomic-park", with_waiting);
            let before = snapshot(folder.path());
            let run = switch_off_with(&file, &["--id", "w", "--write"], NOW, &mut FailAt::new(step, 1));
            assert_told(&run, "The waiting change", step, &waiting_of(&file), &format!("parking, {step:?}, waiting file there {with_waiting}"));
            assert_eq!(snapshot(folder.path()), before, "parking, {step:?}, waiting file there {with_waiting}: nothing came, went or changed");
            let again = switch_off(&file, &["--id", "w", "--write"], NOW);
            assert_eq!((again.code, again.err.as_str()), (0, ""), "parking, {step:?}, again");
            assert_eq!(records_of(&file).len(), if with_waiting { 2 } else { 1 }, "parking, {step:?}, again");
        }
    }

    // `settle` lands through the same step: when the zip cannot be saved, the waiting file is not touched.
    for step in STEPS {
        let (folder, file) = folder_with("atomic-settle", false);
        write_waiting(&file, &[record("a", "2026-11-02T13:00:00Z", "2026-11-02T14:00:00Z")]);
        let before = snapshot(folder.path());
        let run = settle_with(&file, &["--write"], NOW, &mut FailAt::new(step, 1));
        assert_told(&run, "The change", step, &file, &format!("settle, {step:?}"));
        assert_eq!(snapshot(folder.path()), before, "settle, {step:?}: both files and the folder as they were");
        let again = settle(&file, &["--write"], NOW);
        assert_eq!((again.code, again.err.as_str()), (0, ""), "settle, {step:?}, again");
        assert_eq!(block_of(&file, "a")["on"], false, "settle, {step:?}, again");
        assert!(records_of(&file).is_empty(), "settle, {step:?}, again: the change was cleared");
    }
}

#[cfg(unix)]
#[test]
fn a_folder_nobody_may_write_stops_a_save_before_a_temporary_file_is_made() {
    for (what, id, subject, kind) in [("landing", "a", "The change", "settings file"), ("parking", "w", "The waiting change", "waiting file")] {
        let (folder, file) = folder_with("atomic-readonly", false);
        let left = if kind == "waiting file" { waiting_of(&file) } else { file.clone() };
        let before = snapshot(folder.path());
        folder.set_mode(0o555);
        if std::fs::File::create(folder.path().join("probe")).is_err() {
            let run = switch_off(&file, &["--id", id, "--write"], NOW);
            assert_eq!((run.code, run.out.as_str()), (1, ""), "{what}: {}", run.err);
            assert!(run.err.starts_with(&format!("{subject} could not be saved: a temporary file could not be made beside it (Permission denied")), "{what}: {}", run.err);
            assert!(run.err.ends_with(&format!("The {kind} {} was left as it was.\n", shown(&left.to_string_lossy()))) && run.err.lines().count() == 1, "{what}: {}", run.err);
            folder.set_mode(0o755);
            assert_eq!(snapshot(folder.path()), before, "{what}: nothing came, went or changed");
        }
        folder.set_mode(0o755);
    }
}
