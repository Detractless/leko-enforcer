//! `leko settle` shows a draft of what is due and saves nothing; with `--write` each due change
//! switches its block off in the zip and its record leaves the waiting file, the changes that are not
//! due stay as they were, and the zip is saved before the waiting file (AC-013).

mod delay_runs;
#[path = "../../core/tests/delay_sets/mod.rs"]
mod delay_sets;
mod run_capture;
mod scratch_folder;
#[path = "../../core/tests/zip_writer/mod.rs"]
mod zip_writer;

use delay_runs::{assert_core_reads, assert_switched_off, block_of, entries_except, entries_of, record, records_of, settle, settle_with, snapshot, waiting_of, write_waiting, write_zip, FailAt, NOW};
use delay_sets::{locked_block, wait};
use leko_cli::session::session_report::{quoted, shown};
use scratch_folder::Scratch;
use serde_json::Value;

const NOT_DUE_B: &str = "The block \"b\" is not due yet: it may be switched off from 2026-11-03T09:00:00-05:00 (America/New_York), held by: the block's own lock (a wait of 24 hours).\n";

#[test]
fn write_lands_only_the_due_changes_and_keeps_the_rest_waiting() {
    let folder = Scratch::new("settle-land");
    // `a` and `c` have no lock, `d` waits an hour (asked for at 13:30 UTC, so due at 14:30), `b` waits a day.
    let blocks = [locked_block("a", Value::Null), locked_block("b", wait(24)), locked_block("c", Value::Null), locked_block("d", wait(1)), locked_block("e", Value::Null)];
    let file = write_zip(folder.path(), "s.zip", &blocks, &[]);
    let (due_a, parked_b, due_c, due_d) = (
        record("a", "2026-11-02T13:00:00Z", "2026-11-02T14:00:00Z"),
        record("b", "2026-11-02T14:00:00Z", "2026-11-03T14:00:00Z"),
        record("c", "2026-11-02T12:00:00Z", "2026-11-02T12:00:00Z"),
        record("d", "2026-11-02T13:30:00Z", "2026-11-02T14:30:00Z"),
    );
    write_waiting(&file, &[due_a, parked_b.clone(), due_c, due_d]);
    let (before_entries, before_a, before_c, before_d) = (entries_of(&file), block_of(&file, "a"), block_of(&file, "c"), block_of(&file, "d"));
    let command = format!("leko settle {}", quoted(&file.to_string_lossy()));
    let lines = |done: bool| {
        let (verb_a, verb_c, verb_d) = if done { ("is switched off.", "is switched off.", "is switched off.") } else { ("is due and would be switched off.", "is due and would be switched off.", "is due and would be switched off.") };
        format!("The block \"a\" {verb_a}\n{NOT_DUE_B}The block \"c\" {verb_c}\nThe block \"d\" {verb_d}\n")
    };

    // The draft says what would happen and saves nothing.
    let files_before = snapshot(folder.path());
    let draft = settle(&file, &[], NOW);
    assert_eq!((draft.code, draft.err.as_str()), (0, ""));
    assert_eq!(draft.out, format!("{}Draft: nothing was saved. To save this, run: {command} --write\n", lines(false)));
    assert_eq!(snapshot(folder.path()), files_before, "the draft saved nothing");

    // --write: the due blocks are off, the rest of the zip is as it was, and only `b` is still waiting.
    let mut steps = FailAt::recording();
    let run = settle_with(&file, &["--write"], NOW, &mut steps);
    assert_eq!((run.code, run.err.as_str()), (0, ""));
    let waiting = waiting_of(&file);
    assert_eq!(run.out, format!("{}Saved: 3 switched off in {}, 0 cleared and 1 still waiting in {}.\n", lines(true), shown(&file.to_string_lossy()), shown(&waiting.to_string_lossy())));
    assert_eq!(steps.moved_to, ["s.zip", "s.zip.waiting.json"], "the zip is saved first and the waiting file second");
    assert_core_reads(&file);
    assert_switched_off(&before_a, &block_of(&file, "a"));
    assert_switched_off(&before_c, &block_of(&file, "c"));
    assert_switched_off(&before_d, &block_of(&file, "d"));
    let untouched = ["a", "c", "d"];
    assert_eq!(entries_except(&file, &untouched), before_entries.iter().filter(|(name, _)| !untouched.iter().any(|id| *name == format!("blocks/{id}.json"))).cloned().collect::<Vec<_>>(), "no other file of the zip changed");
    assert_eq!(records_of(&file), [parked_b.clone()], "`b` is still parked, exactly as it was");
    assert_eq!(folder.names(), ["s.zip", "s.zip.waiting.json"]);

    // Run again: nothing lands twice, nothing is rewritten.
    let after_first = snapshot(folder.path());
    for flags in [&[][..], &["--write"]] {
        let again = settle(&file, flags, NOW);
        assert_eq!((again.code, again.err.as_str()), (0, ""), "{flags:?}");
        assert_eq!(again.out, format!("{NOT_DUE_B}Nothing is due yet. Nothing was saved.\n"), "{flags:?}");
        assert_eq!(snapshot(folder.path()), after_first, "{flags:?}");
    }

    // When the last one is due it lands, and the waiting file stays, with an empty list.
    let before_b = block_of(&file, "b");
    let later = settle(&file, &["--write"], "2026-11-03T14:00:00Z");
    assert_eq!((later.code, later.err.as_str()), (0, ""));
    assert!(later.out.starts_with("The block \"b\" is switched off.\nSaved: 1 switched off in "), "{}", later.out);
    assert!(later.out.contains(", 0 cleared and 0 still waiting in "), "{}", later.out);
    assert_switched_off(&before_b, &block_of(&file, "b"));
    let text = std::fs::read_to_string(&waiting).unwrap();
    let json: Value = serde_json::from_str(&text).unwrap();
    assert_eq!((json["format"].clone(), json["changes"].clone()), (Value::from(1), Value::Array(vec![])), "{text}");
    let done = snapshot(folder.path());
    let nothing = settle(&file, &["--write"], "2026-11-03T14:00:00Z");
    assert_eq!((nothing.code, nothing.out.as_str(), nothing.err.as_str()), (0, "Nothing is waiting. Nothing was saved.\n", ""));
    assert_eq!(snapshot(folder.path()), done, "an empty list is left alone");
    assert_eq!(block_of(&file, "e")["on"], Value::Null, "a block nobody asked about was never touched");
}
