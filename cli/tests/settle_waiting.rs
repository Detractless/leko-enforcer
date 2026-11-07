//! `leko settle` with nothing due changes nothing (AC-012): no waiting file, or an empty list, says
//! "Nothing is waiting" and makes no file; changes that are not yet due are listed one line each (block,
//! the local moment it may land, and why it waits) and nothing is saved, even with `--write`; a clock
//! set back before the moment a change was asked for does not make it due.

mod delay_runs;
#[path = "../../core/tests/delay_sets/mod.rs"]
mod delay_sets;
mod run_capture;
mod scratch_folder;
#[path = "../../core/tests/zip_writer/mod.rs"]
mod zip_writer;

use delay_runs::{record, settle, snapshot, write_waiting, write_zip, NOW};
use delay_sets::{locked_block, wait};
use scratch_folder::Scratch;
use serde_json::Value;
use std::path::Path;

/// Settle with and without `--write`: both must give `expected` on stdout, exit 0 and save nothing.
fn assert_nothing_changes(folder: &Scratch, file: &Path, now: &str, expected: &str, what: &str) {
    let before = snapshot(folder.path());
    for flags in [&[][..], &["--write"]] {
        let run = settle(file, flags, now);
        assert_eq!((run.code, run.err.as_str()), (0, ""), "{what} {flags:?}");
        assert_eq!(run.out, expected, "{what} {flags:?}");
        assert_eq!(snapshot(folder.path()), before, "{what} {flags:?}: no file was written, made or removed");
    }
}

#[test]
fn settle_lists_what_is_not_due_and_changes_nothing() {
    let folder = Scratch::new("settle-waiting");
    let file = write_zip(folder.path(), "s.zip", &[locked_block("a", wait(24)), locked_block("b", wait(48)), locked_block("c", Value::Null)], &[]);
    let nothing = "Nothing is waiting. Nothing was saved.\n";

    // No waiting file: nothing is waiting, and no empty file is made.
    assert_nothing_changes(&folder, &file, NOW, nothing, "no waiting file");
    assert_eq!(folder.names(), ["s.zip"]);

    // An empty list is nothing waiting, and the file is not rewritten.
    write_waiting(&file, &[]);
    assert_nothing_changes(&folder, &file, NOW, nothing, "an empty list");

    // Two changes that are not due: one line each, in the order parked, with the local moment and why.
    write_waiting(&file, &[record("a", "2026-11-02T14:00:00Z", "2026-11-03T14:00:00Z"), record("b", "2026-11-02T13:00:00Z", "2026-11-04T13:00:00Z")]);
    let listed = concat!(
        "The block \"a\" is not due yet: it may be switched off from 2026-11-03T09:00:00-05:00 (America/New_York), held by: the block's own lock (a wait of 24 hours).\n",
        "The block \"b\" is not due yet: it may be switched off from 2026-11-04T08:00:00-05:00 (America/New_York), held by: the block's own lock (a wait of 48 hours).\n",
        "Nothing is due yet. Nothing was saved.\n"
    );
    assert_nothing_changes(&folder, &file, NOW, listed, "two changes that are not due");

    // One second before the first is due is still not due.
    assert_nothing_changes(&folder, &file, "2026-11-03T13:59:59Z", listed, "one second early");

    // The clock was set back: the change was asked for at 18:00 UTC and it is 15:00 now.
    write_waiting(&file, &[record("c", "2026-11-02T18:00:00Z", "2026-11-02T18:00:00Z")]);
    let set_back = concat!(
        "The block \"c\" is not due yet: it may be switched off from 2026-11-02T13:00:00-05:00 (America/New_York), held by: the moment saved when the change was asked for.\n",
        "Nothing is due yet. Nothing was saved.\n"
    );
    assert_nothing_changes(&folder, &file, NOW, set_back, "a clock set back");
}
