//! With `--write` and a change that must wait, only the waiting file is written (AC-005): it sits
//! beside the settings file, holds the block, the moment it was asked for and the moment it may land
//! in UTC, and the settings zip is byte for byte the same.

mod delay_runs;
#[path = "../../core/tests/delay_sets/mod.rs"]
mod delay_sets;
mod run_capture;
mod scratch_folder;
#[path = "../../core/tests/zip_writer/mod.rs"]
mod zip_writer;

use delay_runs::{record, records_of, switch_off, switch_off_in, switch_off_ms, waiting_of, write_waiting, write_zip, NOW};
use run_capture::moment;
use delay_sets::{locked_block, routine_holding, wait};
use scratch_folder::Scratch;
use serde_json::Value;

#[test]
fn write_parks_a_locked_change_and_the_settings_zip_is_not_touched() {
    let folder = Scratch::new("park");
    let routine = routine_holding("r", &["held"], wait(48));
    let file = write_zip(folder.path(), "s.zip", &[locked_block("slow", wait(24)), locked_block("held", Value::Null), locked_block("later", wait(72))], &[routine]);
    let zip_before = std::fs::read(&file).unwrap();

    // The first change makes the waiting file; the zip is untouched.
    let run = switch_off_ms(&file, &["--id", "slow", "--write"], moment(NOW) + 789);
    assert_eq!((run.code, run.err.as_str()), (0, ""));
    assert!(run.out.starts_with("Waiting: the block \"slow\" is not switched off yet."), "{}", run.out);
    assert!(run.out.contains("from 2026-11-03T10:00:00-05:00 (America/New_York)"), "{}", run.out);
    assert!(run.out.contains("held by: the block's own lock (a wait of 24 hours)"), "{}", run.out);
    assert!(run.out.contains(&format!("parked in {}", waiting_of(&file).display())), "{}", run.out);
    assert_eq!(std::fs::read(&file).unwrap(), zip_before, "the settings zip is byte for byte the same");
    assert_eq!(folder.names(), ["s.zip", "s.zip.waiting.json"], "the waiting file is beside the settings file, and nothing else is made");
    assert_eq!(records_of(&file), [record("slow", "2026-11-02T15:00:00Z", "2026-11-03T15:00:00Z")], "the request moment is the whole second, both moments are UTC");

    // A second block is added to the file; the first record is kept as it was.
    let run = switch_off(&file, &["--id", "held", "--write"], "2026-11-02T16:00:00Z");
    assert_eq!((run.code, run.err.as_str()), (0, ""));
    assert!(run.out.contains("the lock of the routine \"r\", which holds the block (a wait of 48 hours)"), "{}", run.out);
    assert_eq!(
        records_of(&file),
        [record("slow", "2026-11-02T15:00:00Z", "2026-11-03T15:00:00Z"), record("held", "2026-11-02T16:00:00Z", "2026-11-04T16:00:00Z")]
    );
    assert_eq!(std::fs::read(&file).unwrap(), zip_before);

    // The zone the user sees does not change what is saved: the moments are UTC whatever the zone.
    let other = Scratch::new("park-tokyo");
    let tokyo = write_zip(other.path(), "s.zip", &[locked_block("slow", wait(24))], &[]);
    let run = switch_off_in(&tokyo, &["--id", "slow", "--write", "--zone", "Asia/Tokyo"], NOW, None);
    assert_eq!((run.code, run.err.as_str()), (0, ""));
    assert!(run.out.contains("from 2026-11-04T00:00:00+09:00 (Asia/Tokyo)"), "{}", run.out);
    assert_eq!(records_of(&tokyo), [record("slow", "2026-11-02T15:00:00Z", "2026-11-03T15:00:00Z")]);

    // A waiting file written by hand in the form this version writes is added to, not replaced.
    let hand = Scratch::new("park-hand");
    let by_hand = write_zip(hand.path(), "s.zip", &[locked_block("slow", wait(24))], &[]);
    write_waiting(&by_hand, &[record("older", "2026-10-01T00:00:00Z", "2026-10-02T00:00:00Z")]);
    let run = switch_off(&by_hand, &["--id", "slow", "--write"], NOW);
    assert_eq!((run.code, run.err.as_str()), (0, ""));
    assert_eq!(records_of(&by_hand).len(), 2);
    assert_eq!(records_of(&by_hand)[0], record("older", "2026-10-01T00:00:00Z", "2026-10-02T00:00:00Z"));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(std::fs::metadata(waiting_of(&tokyo)).unwrap().permissions().mode() & 0o7777, 0o644, "a new waiting file is readable by others");
    }
}
