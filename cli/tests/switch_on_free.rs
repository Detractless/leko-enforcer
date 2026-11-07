//! A switch-on is never held back (AC-005): whatever lock the block has, and whatever lock a routine
//! that holds it has, `leko switch-on --write` lands at once, with no wait, no partner's key, no moment
//! and no parked change. A block with a long wait, a seal, a key or change hours outside their window is
//! switched on in the same one run, and the answer never mentions a wait or a key.

mod delay_runs;
#[path = "../../core/tests/delay_sets/mod.rs"]
mod delay_sets;
mod run_capture;
mod scratch_folder;
#[path = "../../core/tests/zip_writer/mod.rs"]
mod zip_writer;

use delay_runs::{assert_core_reads, assert_switched_on, block_of, entries_except, snapshot, switch_on, waiting_of, write_zip};
use delay_sets::{change_hours, locked_block, routine_holding, sealed_at, sealed_until, wait, with_partner_key};
use leko_cli::session::session_report::shown;
use scratch_folder::Scratch;
use serde_json::{json, Value};

fn off_with(id: &str, lock: Value) -> Value {
    let mut file = locked_block(id, lock);
    file["on"] = json!(false);
    file
}

/// Every lock the block itself can carry, with a word for the failure message.
fn own_locks() -> Vec<(&'static str, Value)> {
    vec![
        ("no lock", Value::Null),
        ("open", json!({"level": "open"})),
        ("a wait of a day", wait(24)),
        ("a wait of a year", wait(24 * 365)),
        ("change hours outside their window", change_hours("08:00", "09:00", 24)),
        ("change hours that never leave a moment", change_hours("08:00", "09:00", 0)),
        ("a seal until a far day", sealed_until("2999-01-01")),
        ("a seal until a far moment", sealed_at("2999-01-01T00:00:00Z")),
        ("a partner's key on a wait", with_partner_key(wait(24))),
        ("a partner's key on a seal", with_partner_key(sealed_until("2999-01-01"))),
    ]
}

/// The run saved at once and said only that: no wait, no key, nothing parked.
fn assert_landed(run: &run_capture::Outcome, file: &std::path::Path, what: &str) {
    let saved = format!("Saved: the block \"a\" is switched on in {}.\n", shown(&file.to_string_lossy()));
    assert_eq!((run.code, run.err.as_str(), run.out.as_str()), (0, "", saved.as_str()), "{what}");
}

#[test]
fn a_switch_on_is_never_held_by_a_lock_or_a_key() {
    for (what, lock) in own_locks() {
        let folder = Scratch::new("switch-on-free");
        let file = write_zip(folder.path(), "s.zip", &[off_with("a", lock), off_with("z", wait(24))], &[]);
        let (before, others) = (block_of(&file, "a"), entries_except(&file, &["a"]));

        let draft = switch_on(&file, &["--id", "a"]);
        assert!(draft.out.starts_with("Draft: the block \"a\" would be switched on now."), "{what}: {}", draft.out);
        let run = switch_on(&file, &["--id", "a", "--write"]);
        assert_landed(&run, &file, what);
        assert_core_reads(&file);
        assert_switched_on(&before, &block_of(&file, "a"));
        assert_eq!(entries_except(&file, &["a"]), others, "{what}: no other file of the zip changed");
        assert_eq!(folder.names(), ["s.zip"], "{what}: nothing parked, nothing left behind");
    }
}

#[test]
fn a_block_held_by_a_routine_with_a_closed_lock_is_switched_on_at_once() {
    let routine_locks = [("a long wait", wait(24 * 365)), ("a seal", sealed_until("2999-01-01")), ("a partner's key", with_partner_key(wait(24))), ("closed change hours", change_hours("08:00", "09:00", 24))];
    for (what, lock) in routine_locks {
        let folder = Scratch::new("switch-on-free-routine");
        let file = write_zip(folder.path(), "s.zip", &[off_with("a", wait(24))], &[routine_holding("w", &["a"], lock)]);
        let before = block_of(&file, "a");
        let run = switch_on(&file, &["--id", "a", "--write"]);
        assert_landed(&run, &file, what);
        assert_core_reads(&file);
        assert_switched_on(&before, &block_of(&file, "a"));
        assert!(!waiting_of(&file).exists(), "{what}: nothing was parked");
    }
}

#[test]
fn a_switch_on_asks_for_no_moment_so_the_same_answer_comes_whenever_it_is_run() {
    let folder = Scratch::new("switch-on-free-moment");
    let file = write_zip(folder.path(), "s.zip", &[off_with("a", sealed_until("2999-01-01"))], &[]);
    let before = snapshot(folder.path());
    let first = switch_on(&file, &["--id", "a"]);
    let second = switch_on(&file, &["--id", "a"]);
    assert_eq!((first.code, &first.out), (0, &second.out), "a draft says the same every time");
    assert_eq!(snapshot(folder.path()), before);
}
