//! `leko cancel FILE --id ID --write` takes only that block's record out of the waiting file (AC-004):
//! the other records are exactly as the core's writer would write them, in the same order; the list
//! is kept when it empties; the settings zip is byte for byte the same; the sentence says
//! "Cancelled"; and the saved file passes the core's reader.

mod delay_runs;
#[path = "../../core/tests/delay_sets/mod.rs"]
mod delay_sets;
mod run_capture;
mod scratch_folder;
#[path = "../../core/tests/zip_writer/mod.rs"]
mod zip_writer;

use delay_runs::{cancel, record, records_of, snapshot, waiting_of, write_waiting, write_zip};
use delay_sets::{locked_block, wait};
use leko_cli::session::session_report::quoted;
use leko_core::delayed_changes::waiting_file::{read_waiting, write_waiting as core_text};
use scratch_folder::Scratch;
use serde_json::Value;

fn cancelled(id: &str, waiting_file: &std::path::Path) -> String {
    format!("Cancelled: the waiting change for the block \"{id}\" is taken back and removed from {}.\n", quoted(&waiting_file.to_string_lossy()))
}

#[test]
fn write_removes_only_that_record_and_never_touches_the_settings_zip() {
    let folder = Scratch::new("cancel-write");
    let blocks: Vec<Value> = ["a", "ab", "b"].iter().map(|id| locked_block(id, wait(24))).collect();
    let file = write_zip(folder.path(), "s.zip", &blocks, &[]);
    let (first, middle, last) = (record("a", "2026-11-02T14:00:00Z", "2026-11-03T14:00:00Z"), record("ab", "2026-11-02T13:00:00Z", "2026-11-03T13:00:00Z"), record("b", "2026-11-02T12:00:00Z", "2026-11-03T12:00:00Z"));
    write_waiting(&file, &[first.clone(), middle.clone(), last.clone()]);
    let waiting = waiting_of(&file);
    let zip_before = std::fs::read(&file).expect("the zip");

    // The middle record goes. The file is what the core writes for the other two, in their order.
    let before_text = std::fs::read_to_string(&waiting).expect("text");
    let both = read_waiting(&before_text).expect("the core reads it");
    let run = cancel(&file, &["--id", "ab", "--write"]);
    assert_eq!((run.code, run.err.as_str(), run.out.as_str()), (0, "", cancelled("ab", &waiting).as_str()));
    assert_eq!(records_of(&file), [first.clone(), last.clone()]);
    let expected = core_text(&[both[0].clone(), both[2].clone()]).expect("the core writes it");
    assert_eq!(std::fs::read_to_string(&waiting).unwrap(), expected, "the saved file is the other two records, as the core writes them");
    assert_eq!(read_waiting(&expected).expect("the core reads what was saved").len(), 2);
    assert_eq!(std::fs::read(&file).unwrap(), zip_before, "the settings zip is byte for byte the same");
    assert_eq!(folder.names(), ["s.zip", "s.zip.waiting.json"], "no temporary file is left");

    // `a` is not a prefix match for `ab`, and cancelling the first record keeps the last one.
    let run = cancel(&file, &["--id", "a", "--write"]);
    assert_eq!((run.code, run.err.as_str(), run.out.as_str()), (0, "", cancelled("a", &waiting).as_str()));
    assert_eq!(records_of(&file), [last.clone()]);

    // The last record leaves an empty list: the file is kept, in the form the reader accepts.
    let run = cancel(&file, &["--id", "b", "--write"]);
    assert_eq!((run.code, run.err.as_str(), run.out.as_str()), (0, "", cancelled("b", &waiting).as_str()));
    assert!(waiting.is_file(), "the waiting file is not deleted when the list empties");
    assert_eq!(records_of(&file), Vec::<Value>::new());
    assert_eq!(read_waiting(&std::fs::read_to_string(&waiting).unwrap()).expect("the core reads it").len(), 0);
    assert_eq!(std::fs::read(&file).unwrap(), zip_before, "the zip is still the same");
    assert_eq!(folder.names(), ["s.zip", "s.zip.waiting.json"]);
}

#[test]
fn cancelling_one_blocks_change_leaves_the_others_files_in_the_folder_alone() {
    let folder = Scratch::new("cancel-write-other");
    let file = write_zip(folder.path(), "s.zip", &[locked_block("a", wait(24))], &[]);
    write_waiting(&file, &[record("a", "2026-11-02T14:00:00Z", "2026-11-03T14:00:00Z")]);
    std::fs::write(folder.path().join("other.zip.waiting.json"), b"not mine").unwrap();
    let before = snapshot(folder.path());
    assert_eq!(cancel(&file, &["--id", "a", "--write"]).code, 0);
    let after = snapshot(folder.path());
    let changed: Vec<&str> = after.iter().zip(&before).filter(|(now, was)| now != was).map(|(now, _)| now.0.as_str()).collect();
    assert_eq!(changed, ["s.zip.waiting.json"], "only the waiting file beside this settings file changed");
}
