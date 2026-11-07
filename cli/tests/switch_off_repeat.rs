//! Running a command twice is safe and says what it did (AC-006): a block that is already off is
//! reported as off and nothing is saved; a block that is already waiting is reported as waiting with
//! the moment it may land, and the waiting file is left exactly as it was.

mod delay_runs;
#[path = "../../core/tests/delay_sets/mod.rs"]
mod delay_sets;
mod run_capture;
mod scratch_folder;
#[path = "../../core/tests/zip_writer/mod.rs"]
mod zip_writer;

use delay_runs::{records_of, snapshot, switch_off, waiting_of, write_zip, NOW};
use delay_sets::{locked_block, off_block, wait};
use scratch_folder::Scratch;
use serde_json::Value;

#[test]
fn a_second_switch_off_changes_nothing_and_says_why() {
    // A block that is already off: with and without --write, exit 0, nothing saved.
    let folder = Scratch::new("repeat-off");
    let file = write_zip(folder.path(), "s.zip", &[off_block("gone"), locked_block("a", Value::Null)], &[]);
    let before = snapshot(folder.path());
    for flags in [&["--id", "gone"][..], &["--id", "gone", "--write"]] {
        let run = switch_off(&file, flags, NOW);
        assert_eq!((run.code, run.err.as_str()), (0, ""), "{flags:?}");
        assert_eq!(run.out, "The block \"gone\" is already off. Nothing was saved.\n", "{flags:?}");
        assert!(!run.out.contains("Saved"));
        assert_eq!(snapshot(folder.path()), before, "{flags:?}: not even rewritten");
    }

    // A block switched off by --write is off the second time too, and the file is not rewritten.
    let run = switch_off(&file, &["--id", "a", "--write"], NOW);
    assert!(run.out.starts_with("Saved:"), "{}", run.out);
    let after_first = snapshot(folder.path());
    assert_ne!(after_first, before);
    let run = switch_off(&file, &["--id", "a", "--write"], NOW);
    assert_eq!((run.code, run.err.as_str(), run.out.as_str()), (0, "", "The block \"a\" is already off. Nothing was saved.\n"));
    assert_eq!(snapshot(folder.path()), after_first);

    // A block that is already waiting: the same words, the same moment, and the file is not moved.
    let folder = Scratch::new("repeat-wait");
    let file = write_zip(folder.path(), "s.zip", &[locked_block("slow", wait(24))], &[]);
    let run = switch_off(&file, &["--id", "slow", "--write"], NOW);
    assert!(run.out.starts_with("Waiting:"), "{}", run.out);
    let parked = snapshot(folder.path());
    let record = records_of(&file);
    for (flags, at) in [(&["--id", "slow", "--write"][..], "2026-11-02T16:00:00Z"), (&["--id", "slow"], "2026-11-02T20:00:00Z"), (&["--id", "slow", "--write"], "2026-11-03T14:59:59Z")] {
        let run = switch_off(&file, flags, at);
        assert_eq!((run.code, run.err.as_str()), (0, ""), "{flags:?} at {at}");
        assert_eq!(
            run.out,
            "The block \"slow\" is already waiting: it may be switched off from 2026-11-03T10:00:00-05:00 (America/New_York). Nothing was saved.\n",
            "{flags:?} at {at}"
        );
        assert_eq!(snapshot(folder.path()), parked, "{flags:?} at {at}: no second record, no restarted wait");
        assert_eq!(records_of(&file), record);
    }
    assert_eq!(std::fs::read(waiting_of(&file)).unwrap(), parked.iter().find(|(name, _)| name == "s.zip.waiting.json").unwrap().1);

    // Once it is due the sentence says so and names the command that lands it.
    let run = switch_off(&file, &["--id", "slow"], "2026-11-03T15:00:00Z");
    assert_eq!((run.code, run.err.as_str()), (0, ""));
    assert!(run.out.contains("It is due now: run leko settle ") && run.out.ends_with(" --write\n"), "{}", run.out);
    assert_eq!(snapshot(folder.path()), parked);
}
