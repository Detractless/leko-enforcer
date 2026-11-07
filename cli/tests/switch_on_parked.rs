//! A parked switch-off of the same block stops a switch-on (AC-008), because `leko settle` would switch
//! the block off again later: exit 1 and one sentence saying so and naming `leko cancel FILE --id ID
//! --write`, for the draft and for `--write`, whether the block is off or on; nothing is saved. A parked
//! change of another block (or one whose id only starts the same) does not stop it, and once the change
//! is cancelled the switch-on goes through.

mod delay_runs;
#[path = "../../core/tests/delay_sets/mod.rs"]
mod delay_sets;
mod run_capture;
mod scratch_folder;
#[path = "../../core/tests/zip_writer/mod.rs"]
mod zip_writer;

use delay_runs::{block_of, cancel, one_line, record, records_of, snapshot, switch_on, waiting_of, write_waiting, write_zip};
use delay_sets::{locked_block, off_block, wait};
use leko_cli::session::session_report::quoted;
use scratch_folder::Scratch;
use std::path::Path;

fn parked(id: &str) -> serde_json::Value {
    record(id, "2026-11-02T14:00:00Z", "2026-11-03T14:00:00Z")
}

fn stop_for(id: &str, file: &Path) -> String {
    format!(
        "The block \"{id}\" has a switch-off waiting, which would undo a switch-on. Cancel it first: leko cancel {} --id {id} --write. Nothing was saved.\n",
        quoted(&file.to_string_lossy())
    )
}

#[test]
fn a_parked_switch_off_of_the_same_block_stops_a_switch_on_until_it_is_cancelled() {
    for name in ["s.zip", "my settings.zip"] {
        let folder = Scratch::new("switch-on-parked");
        // `a` is off with a wait, `on` is on with one (it was parked while it was on), `ab` and `b` are others.
        let mut on_one = locked_block("on", wait(24));
        on_one["on"] = serde_json::json!(true);
        let file = write_zip(folder.path(), name, &[off_block("a"), off_block("ab"), off_block("b"), on_one], &[]);
        write_waiting(&file, &[parked("a"), parked("b"), parked("on")]);
        let before = snapshot(folder.path());

        for id in ["a", "on"] {
            for flags in [&["--id", id][..], &["--id", id, "--write"]] {
                let run = switch_on(&file, flags);
                assert_eq!((run.code, run.out.as_str()), (1, ""), "{name} {id} {flags:?}: {}", run.err);
                assert_eq!(one_line(&run.err), stop_for(id, &file), "{name} {id} {flags:?}");
                assert_eq!(snapshot(folder.path()), before, "{name} {id} {flags:?}: nothing was saved");
            }
        }

        // The command the sentence names is the one that clears the way, and then the switch-on goes through.
        let cancelled = cancel(&file, &["--id", "a", "--write"]);
        assert_eq!((cancelled.code, cancelled.err.as_str()), (0, ""), "{name}: {}", cancelled.out);
        assert_eq!(records_of(&file), [parked("b"), parked("on")], "{name}");
        let draft = switch_on(&file, &["--id", "a"]);
        assert!(draft.code == 0 && draft.out.starts_with("Draft: the block \"a\" would be switched on now."), "{name}: {}", draft.out);
        let run = switch_on(&file, &["--id", "a", "--write"]);
        assert_eq!((run.code, run.err.as_str()), (0, ""), "{name}");
        assert_eq!(block_of(&file, "a")["on"], true, "{name}");
        assert_eq!(block_of(&file, "ab")["on"], false, "{name}: `ab` is still off");
        assert_eq!(records_of(&file), [parked("b"), parked("on")], "{name}: the other blocks' parked changes are as they were");
    }
}

#[test]
fn a_parked_change_of_another_block_does_not_stop_a_switch_on() {
    let folder = Scratch::new("switch-on-parked-other");
    let file = write_zip(folder.path(), "s.zip", &[off_block("a"), off_block("ab"), off_block("b")], &[]);
    write_waiting(&file, &[parked("ab"), parked("b")]);
    let waiting_before = std::fs::read(waiting_of(&file)).expect("the waiting file");
    let run = switch_on(&file, &["--id", "a", "--write"]);
    assert_eq!((run.code, run.err.as_str()), (0, ""), "{}", run.out);
    assert!(run.out.starts_with("Saved: the block \"a\" is switched on in "), "{}", run.out);
    assert_eq!(block_of(&file, "a")["on"], true);
    assert_eq!(std::fs::read(waiting_of(&file)).unwrap(), waiting_before, "the waiting file is as it was");
}

#[test]
fn a_waiting_file_with_nothing_for_the_block_or_none_at_all_is_no_stop() {
    let folder = Scratch::new("switch-on-parked-none");
    let file = write_zip(folder.path(), "s.zip", &[off_block("a")], &[]);
    write_waiting(&file, &[]);
    assert_eq!(switch_on(&file, &["--id", "a"]).code, 0, "an empty list");
    std::fs::remove_file(waiting_of(&file)).unwrap();
    assert_eq!(switch_on(&file, &["--id", "a", "--write"]).code, 0, "no waiting file");
    assert!(!waiting_of(&file).exists(), "and none is made");
}
