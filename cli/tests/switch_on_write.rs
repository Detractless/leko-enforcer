//! `leko switch-on FILE --id ID --write` changes only that block's `on` field (AC-004): the zip is
//! replaced by one the core reads again, in which the block is on and every other file of the zip is
//! byte for byte the same; the waiting file beside it is not read for a change, not rewritten and not
//! made; no temporary file is left.

mod delay_runs;
#[path = "../../core/tests/delay_sets/mod.rs"]
mod delay_sets;
mod run_capture;
mod scratch_folder;
#[path = "../../core/tests/zip_writer/mod.rs"]
mod zip_writer;

use delay_runs::{assert_core_reads, assert_switched_on, block_of, entries_except, record, snapshot, switch_on, waiting_of, write_waiting, write_zip};
use delay_sets::{locked_block, off_block, wait};
use leko_cli::session::session_report::shown;
use scratch_folder::Scratch;
use serde_json::{json, Value};

fn off_with(id: &str, lock: Value) -> Value {
    let mut file = locked_block(id, lock);
    file["on"] = json!(false);
    file
}

fn saved(id: &str, file: &std::path::Path) -> String {
    format!("Saved: the block \"{id}\" is switched on in {}.\n", shown(&file.to_string_lossy()))
}

#[test]
fn write_changes_only_that_blocks_on_field_and_leaves_the_waiting_file_alone() {
    let folder = Scratch::new("switch-on-write");
    let blocks = vec![off_block("a"), off_block("ab"), off_with("b", wait(24)), locked_block("c", wait(24))];
    let file = write_zip(folder.path(), "s.zip", &blocks, &[]);
    write_waiting(&file, &[record("ab", "2026-11-02T14:00:00Z", "2026-11-03T14:00:00Z"), record("c", "2026-11-02T13:00:00Z", "2026-11-03T13:00:00Z")]);
    let waiting_before = std::fs::read(waiting_of(&file)).expect("the waiting file");
    let (a_before, ab_before, b_before) = (block_of(&file, "a"), block_of(&file, "ab"), block_of(&file, "b"));
    let others_before = entries_except(&file, &["a"]);

    let run = switch_on(&file, &["--id", "a", "--write"]);
    assert_eq!((run.code, run.err.as_str(), run.out.as_str()), (0, "", saved("a", &file).as_str()));
    assert_core_reads(&file);
    assert_switched_on(&a_before, &block_of(&file, "a"));
    assert_eq!(entries_except(&file, &["a"]), others_before, "every other file of the zip is byte for byte the same, `ab` (which `a` is a prefix of) too");
    assert_eq!(block_of(&file, "ab"), ab_before);
    assert_eq!(std::fs::read(waiting_of(&file)).unwrap(), waiting_before, "the waiting file is not rewritten");
    assert_eq!(folder.names(), ["s.zip", "s.zip.waiting.json"], "no temporary file is left");

    // A block with a lock is switched on the same way, and its lock is not touched.
    let run = switch_on(&file, &["--id", "b", "--write"]);
    assert_eq!((run.code, run.err.as_str(), run.out.as_str()), (0, "", saved("b", &file).as_str()));
    assert_switched_on(&b_before, &block_of(&file, "b"));
    assert_eq!(block_of(&file, "b")["lock"], b_before["lock"]);
    assert_core_reads(&file);
    assert_eq!(block_of(&file, "ab"), ab_before, "`ab` is still off");
    assert_eq!(std::fs::read(waiting_of(&file)).unwrap(), waiting_before);
}

#[test]
fn no_waiting_file_is_made_and_other_files_in_the_folder_are_left_alone() {
    let folder = Scratch::new("switch-on-write-alone");
    let file = write_zip(folder.path(), "s.zip", &[off_block("a")], &[]);
    std::fs::write(folder.path().join("other.zip.waiting.json"), b"not mine").unwrap();
    let before = snapshot(folder.path());
    assert_eq!(switch_on(&file, &["--id", "a", "--write"]).code, 0);
    let after = snapshot(folder.path());
    let changed: Vec<&str> = after.iter().zip(&before).filter(|(now, was)| now != was).map(|(now, _)| now.0.as_str()).collect();
    assert_eq!(changed, ["s.zip"], "only the settings zip changed");
    assert_eq!(after.len(), before.len(), "no file came and none went");
    assert!(!waiting_of(&file).exists());
}
