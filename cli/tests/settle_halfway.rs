//! When `leko settle` has saved the settings zip and then cannot save the waiting file (AC-015), it says
//! so plainly and exits 1: the due blocks are off, the waiting file is as it was, and running it again
//! clears the records without switching anything twice.

mod parked_change_support;
use parked_change_support::*;

use delay_runs::{assert_core_reads, assert_switched_off, block_of, entries_of, record, records_of, settle, settle_with, snapshot, waiting_of, write_waiting, write_zip, FailAt, Step, NOW};
use delay_sets::{locked_block, wait};
use leko_cli::session::session_report::{quoted, shown};
use scratch_folder::Scratch;
use serde_json::Value;

#[test]
fn a_failed_second_save_is_reported_and_a_rerun_finishes_the_job() {
    for step in [Step::Write, Step::Flush, Step::Rename] {
        let folder = Scratch::new("halfway");
        let file = write_zip(folder.path(), "s.zip", &[locked_block("a", Value::Null), locked_block("b", Value::Null), locked_block("w", wait(24))], &[]);
        write_waiting(&file, &[record("a", "2026-11-02T13:00:00Z", "2026-11-02T14:00:00Z"), record("w", "2026-11-02T14:00:00Z", "2026-11-03T14:00:00Z"), record("b", "2026-11-02T12:00:00Z", "2026-11-02T12:00:00Z")]);
        let (waiting, old_a, old_b) = (waiting_of(&file), block_of(&file, "a"), block_of(&file, "b"));
        let (waiting_before, zip_before) = (std::fs::read(&waiting).unwrap(), entries_of(&file));

        // The first save (the zip) works; the second (the waiting file) fails at `step`.
        let run = settle_with(&file, &["--write"], NOW, &mut FailAt::new(step, 2));
        assert_eq!((run.code, run.out.as_str()), (1, ""), "{step:?}: {}", run.err);
        let command = format!("leko settle {}", quoted(&file.to_string_lossy()));
        let expected = format!(
            "The waiting change could not be saved: {} ({}). The waiting file {} was left as it was. The settings file {} was already saved, so the due blocks are off. Run {command} --write again to clear their waiting changes.\n",
            step.told_as(),
            step.problem(),
            shown(&waiting.to_string_lossy()),
            shown(&file.to_string_lossy())
        );
        assert_eq!(run.err, expected, "{step:?}");
        assert_eq!(run.err.lines().count(), 1, "{step:?}: one line");
        assert_core_reads(&file);
        assert_switched_off(&old_a, &block_of(&file, "a"));
        assert_switched_off(&old_b, &block_of(&file, "b"));
        assert_ne!(block_of(&file, "w")["on"], Value::Bool(false), "{step:?}: the block that is not due stayed on");
        assert_eq!(std::fs::read(&waiting).unwrap(), waiting_before, "{step:?}: the waiting file is byte for byte as it was");
        assert_eq!(folder.names(), ["s.zip", "s.zip.waiting.json"], "{step:?}: no temporary file is left");

        // A rerun with a working disk: the blocks are already off, so their records are cleared, the
        // zip is not touched again, and the change that is not due is still waiting.
        let zip_after_first = entries_of(&file);
        assert_ne!(zip_after_first, zip_before, "{step:?}: the first run did change the zip");
        let on_disk = snapshot(folder.path());
        let zip_bytes = on_disk.iter().find(|(name, _)| name == "s.zip").map(|(_, bytes)| bytes.clone()).unwrap();
        let again = settle(&file, &["--write"], NOW);
        assert_eq!((again.code, again.err.as_str()), (0, ""), "{step:?}");
        assert_eq!(
            again.out,
            format!(
                "The block \"a\" is already off, so its waiting change is cleared.\nThe block \"w\" is not due yet: it may be switched off from 2026-11-03T09:00:00-05:00 (America/New_York), held by: the block's own lock (a wait of 24 hours).\n\
                 The block \"b\" is already off, so its waiting change is cleared.\nSaved: 0 switched off in {}, 2 cleared and 1 still waiting in {}.\n",
                shown(&file.to_string_lossy()),
                shown(&waiting.to_string_lossy())
            ),
            "{step:?}"
        );
        assert_eq!(std::fs::read(&file).unwrap(), zip_bytes, "{step:?}: the zip was not rewritten, so nothing was switched twice");
        assert_eq!(records_of(&file), [record("w", "2026-11-02T14:00:00Z", "2026-11-03T14:00:00Z")], "{step:?}");
    }
}
