//! Cancelling twice, or cancelling nothing, is safe and says what it did (AC-005): with no waiting
//! file, an empty list, or no record for the block, the answer is "Nothing is waiting for the block ..."
//! with the words that nothing was saved, exit 0, and no file is made or changed. An id with a slash,
//! dots or capital letters is just a block nobody parked; it never reaches the file system.

mod delay_runs;
#[path = "../../core/tests/delay_sets/mod.rs"]
mod delay_sets;
mod run_capture;
mod scratch_folder;
#[path = "../../core/tests/zip_writer/mod.rs"]
mod zip_writer;

use delay_runs::{cancel, record, records_of, snapshot, waiting_of, write_waiting, write_zip};
use delay_sets::{locked_block, wait};
use leko_cli::session::session_report::shown;
use scratch_folder::Scratch;

fn nothing_for(id: &str) -> String {
    format!("Nothing is waiting for the block {}. Nothing was saved.\n", shown(id))
}

#[test]
fn a_second_cancel_or_a_cancel_of_nothing_changes_nothing_and_says_why() {
    // No waiting file: the answer is the same with and without --write, and no file is made.
    let folder = Scratch::new("cancel-repeat-none");
    let file = write_zip(folder.path(), "s.zip", &[locked_block("a", wait(24))], &[]);
    let before = snapshot(folder.path());
    for flags in [&["--id", "a"][..], &["--id", "a", "--write"]] {
        let run = cancel(&file, flags);
        assert_eq!((run.code, run.err.as_str(), run.out.as_str()), (0, "", nothing_for("a").as_str()), "{flags:?}");
        assert_eq!(snapshot(folder.path()), before, "{flags:?}: no file made, none changed");
    }
    assert!(!waiting_of(&file).exists(), "a run on nothing makes no waiting file");

    // An empty list, and a list with only another block's record: left byte for byte as they were.
    for records in [vec![], vec![record("b", "2026-11-02T14:00:00Z", "2026-11-03T14:00:00Z")]] {
        write_waiting(&file, &records);
        let before = snapshot(folder.path());
        for flags in [&["--id", "a"][..], &["--id", "a", "--write"]] {
            let run = cancel(&file, flags);
            assert_eq!((run.code, run.err.as_str(), run.out.as_str()), (0, "", nothing_for("a").as_str()), "{} records, {flags:?}", records.len());
            assert_eq!(snapshot(folder.path()), before, "{} records, {flags:?}: not even rewritten", records.len());
        }
    }

    // The same --write typed twice: the second run is a no-op that says so.
    write_waiting(&file, &[record("a", "2026-11-02T14:00:00Z", "2026-11-03T14:00:00Z"), record("b", "2026-11-02T13:00:00Z", "2026-11-03T13:00:00Z")]);
    let first = cancel(&file, &["--id", "a", "--write"]);
    assert!(first.out.starts_with("Cancelled:"), "{}", first.out);
    let after_first = snapshot(folder.path());
    let second = cancel(&file, &["--id", "a", "--write"]);
    assert_eq!((second.code, second.err.as_str(), second.out.as_str()), (0, "", nothing_for("a").as_str()));
    assert_eq!(snapshot(folder.path()), after_first, "the second run changed nothing");
    assert_eq!(records_of(&file).len(), 1, "the other record is still parked");
}

#[test]
fn an_id_shaped_like_a_path_is_a_block_nobody_parked_and_reaches_no_file() {
    let folder = Scratch::new("cancel-repeat-path");
    let file = write_zip(folder.path(), "s.zip", &[locked_block("a", wait(24))], &[]);
    write_waiting(&file, &[record("a", "2026-11-02T14:00:00Z", "2026-11-03T14:00:00Z")]);
    std::fs::create_dir(folder.path().join("inner")).unwrap();
    std::fs::write(folder.path().join("inner").join("a.json"), b"decoy").unwrap();
    let before = (snapshot(folder.path()), snapshot(&folder.path().join("inner")));
    for id in ["../a", "inner/a", "..", ".", "A", "a/", "/etc/passwd", "a\\b", "a b", "a\0b", "a\nb", "*", "~", "$HOME", "ａ", "a.json"] {
        for flags in [&["--id", id][..], &["--id", id, "--write"]] {
            let run = cancel(&file, flags);
            assert_eq!((run.code, run.err.as_str(), run.out.as_str()), (0, "", nothing_for(id).as_str()), "{id:?} {flags:?}");
        }
    }
    assert_eq!((snapshot(folder.path()), snapshot(&folder.path().join("inner"))), before, "nothing came, went or changed");
}
