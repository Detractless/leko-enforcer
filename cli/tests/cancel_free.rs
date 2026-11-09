//! Cancelling needs no wait, no key and no look at the settings (AC-006): it works at once for a block
//! whose lock is a long wait, a seal or a partner's key, for a block that is no longer in the settings,
//! and when the settings zip is missing, empty or not a zip, because only the waiting file is read
//! and saved.

mod parked_change_support;
use parked_change_support::{delay_runs, delay_sets, run_capture, scratch_folder, zip_writer};

use delay_runs::{cancel, record, records_of, snapshot, waiting_of, write_waiting, write_zip};
use delay_sets::{locked_block, sealed_until, with_partner_key, wait};
use scratch_folder::Scratch;
use serde_json::Value;

fn parked(id: &str) -> Value {
    record(id, "2026-11-02T14:00:00Z", "2099-01-01T00:00:00Z")
}

fn cancelled(run: &run_capture::Outcome, id: &str) {
    assert_eq!((run.code, run.err.as_str()), (0, ""), "{id}");
    assert!(run.out.starts_with(&format!("Cancelled: the waiting change for the block \"{id}\" is taken back")), "{id}: {}", run.out);
}

#[test]
fn a_cancel_is_never_held_by_a_lock_and_never_reads_the_settings_zip() {
    let folder = Scratch::new("cancel-free");
    let blocks = [
        locked_block("slow", wait(24 * 365)),
        locked_block("sealed", sealed_until("2999-01-01")),
        locked_block("keyed", with_partner_key(wait(24))),
    ];
    let file = write_zip(folder.path(), "s.zip", &blocks, &[]);
    let zip_before = std::fs::read(&file).unwrap();
    write_waiting(&file, &[parked("slow"), parked("sealed"), parked("keyed"), parked("gone")]);

    // The locks: a year-long wait, a seal for centuries and a partner's key hold nothing back.
    for id in ["slow", "sealed", "keyed"] {
        cancelled(&cancel(&file, &["--id", id, "--write"]), id);
    }
    // A block that is no longer in the settings: its record can still be cleared by hand.
    assert_eq!(records_of(&file), [parked("gone")]);
    cancelled(&cancel(&file, &["--id", "gone", "--write"]), "gone");
    assert_eq!(records_of(&file), Vec::<Value>::new());
    assert_eq!(std::fs::read(&file).unwrap(), zip_before, "the settings zip was never rewritten");
}

#[test]
fn a_missing_empty_or_damaged_settings_zip_does_not_stop_a_cancel() {
    let junk: [(&str, Option<&[u8]>); 4] = [("missing", None), ("empty", Some(b"")), ("not a zip", Some(b"this is not a zip file")), ("cut short", Some(&[0x50, 0x4b, 0x03, 0x04, 0x0a]))];
    for (what, bytes) in junk {
        let folder = Scratch::new("cancel-free-zip");
        let file = folder.path().join("s.zip");
        if let Some(bytes) = bytes {
            std::fs::write(&file, bytes).unwrap();
        }
        write_waiting(&file, &[parked("a"), parked("b")]);
        let zip_before = std::fs::read(&file).ok();
        let draft = cancel(&file, &["--id", "a"]);
        assert_eq!((draft.code, draft.err.as_str()), (0, ""), "{what}");
        assert!(draft.out.starts_with("Draft:"), "{what}: {}", draft.out);
        cancelled(&cancel(&file, &["--id", "a", "--write"]), "a");
        assert_eq!(records_of(&file), [parked("b")], "{what}");
        assert_eq!(std::fs::read(&file).ok(), zip_before, "{what}: the zip is as it was (or still not there)");
        assert!(waiting_of(&file).is_file(), "{what}");
        let names = snapshot(folder.path()).into_iter().map(|(name, _)| name).collect::<Vec<_>>();
        assert_eq!(names.len(), if bytes.is_some() { 2 } else { 1 }, "{what}: {names:?}");
    }
}
