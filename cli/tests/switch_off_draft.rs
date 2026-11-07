//! Without `--write` the command shows one plain draft and writes nothing (AC-003): a block that
//! lands now would be switched off now; a block that must wait would be parked, with the moment it
//! may land on the wall clock of the chosen zone and the lock that makes it wait.

mod delay_runs;
#[path = "../../core/tests/delay_sets/mod.rs"]
mod delay_sets;
mod run_capture;
mod scratch_folder;
#[path = "../../core/tests/zip_writer/mod.rs"]
mod zip_writer;

use delay_runs::{snapshot, switch_off, switch_off_in, write_zip, NOW};
use delay_sets::{locked_block, routine_holding, wait};
use scratch_folder::Scratch;
use serde_json::Value;

#[test]
fn without_the_write_flag_the_draft_says_when_it_lands_and_saves_nothing() {
    let folder = Scratch::new("draft");
    let routine = routine_holding("r", &["held"], wait(72));
    let file = write_zip(folder.path(), "s.zip", &[locked_block("free", Value::Null), locked_block("slow", wait(24)), locked_block("held", Value::Null)], &[routine]);
    let before = snapshot(folder.path());
    let path = file.display().to_string();

    // A block that lands now: the draft says so and how to do it.
    let run = switch_off(&file, &["--id", "free"], NOW);
    assert_eq!((run.code, run.err.as_str()), (0, ""));
    assert!(run.out.starts_with("Draft: the block \"free\" would be switched off now."), "{}", run.out);
    assert!(run.out.contains("Nothing was saved."), "{}", run.out);
    assert!(run.out.contains(&format!("run: leko switch-off {path} --id free --write\n")), "{}", run.out);
    assert!(!run.out.contains("Saved") && !run.out.contains("already off") && !run.out.contains("Waiting"), "{}", run.out);

    // A block that must wait: parked, not made, with the local moment, the zone and the lock.
    let run = switch_off(&file, &["--id", "slow"], NOW);
    assert_eq!((run.code, run.err.as_str()), (0, ""));
    assert!(run.out.contains("the block \"slow\" cannot be switched off yet"), "{}", run.out);
    assert!(run.out.contains("would be parked in") && run.out.contains("s.zip.waiting.json"), "{}", run.out);
    assert!(run.out.contains("from 2026-11-03T10:00:00-05:00 (America/New_York)"), "{}", run.out);
    assert!(run.out.contains("held by: the block's own lock (a wait of 24 hours)"), "{}", run.out);
    assert!(run.out.contains("Nothing was saved.") && run.out.contains(&format!("leko switch-off {path} --id slow --write")), "{}", run.out);
    assert!(!run.out.contains("already off"), "a parked change is not an off block: {}", run.out);

    // The lock of a routine that holds the block is named too.
    let run = switch_off(&file, &["--id", "held"], NOW);
    assert!(run.out.contains("from 2026-11-05T10:00:00-05:00 (America/New_York)"), "{}", run.out);
    assert!(run.out.contains("the lock of the routine \"r\", which holds the block (a wait of 72 hours)"), "{}", run.out);

    // The moment is shown in the zone the user chose: --zone wins over the computer's zone, which
    // is used when there is no --zone.
    let run = switch_off_in(&file, &["--id", "slow", "--zone", "Asia/Tokyo"], NOW, Some("Europe/London"));
    assert!(run.out.contains("from 2026-11-04T00:00:00+09:00 (Asia/Tokyo)"), "{}", run.out);
    let run = switch_off_in(&file, &["--id", "slow"], NOW, Some("America/Chicago"));
    assert!(run.out.contains("from 2026-11-03T09:00:00-06:00 (America/Chicago)"), "{}", run.out);

    // Nothing was written: the file, the folder and the names in it are as they were.
    assert_eq!(snapshot(folder.path()), before);
    assert_eq!(folder.names(), ["s.zip"]);
}
