//! Running a switch-on again, or on a block that is on already, is safe (AC-006): exit 0, one sentence
//! saying the block is already on and that nothing was saved, for the draft and for `--write`; the zip
//! is byte for byte as it was and no file is made. A block with no `on` field is on and is not given
//! one. Nothing reaches a step of the save, so a failing disk does not matter.

mod parked_change_support;
use parked_change_support::{delay_runs, delay_sets, run_capture, scratch_folder, zip_writer};

use delay_runs::{block_of, snapshot, switch_on, switch_on_with, waiting_of, write_zip, FailAt, Step};
use delay_sets::{locked_block, off_block, wait, with_partner_key};
use scratch_folder::Scratch;
use serde_json::{json, Value};

const ALREADY: &str = "The block \"a\" is already on. Nothing was saved.\n";

fn on_with(id: &str, lock: Value, field: Option<bool>) -> Value {
    let mut file = locked_block(id, lock);
    if let Some(on) = field {
        file["on"] = json!(on);
    }
    file
}

#[test]
fn a_second_switch_on_or_a_block_already_on_changes_nothing_and_says_why() {
    // The same `--write` typed twice: the first one switches the block on, the second one is a no-op.
    let folder = Scratch::new("switch-on-repeat");
    let file = write_zip(folder.path(), "s.zip", &[off_block("a")], &[]);
    let first = switch_on(&file, &["--id", "a", "--write"]);
    assert_eq!((first.code, first.err.as_str()), (0, ""));
    assert!(first.out.starts_with("Saved: the block \"a\" is switched on in "), "{}", first.out);
    let after_first = snapshot(folder.path());
    for flags in [&["--id", "a", "--write"][..], &["--id", "a"]] {
        let again = switch_on(&file, flags);
        assert_eq!((again.code, again.err.as_str(), again.out.as_str()), (0, "", ALREADY), "{flags:?}");
        assert_eq!(snapshot(folder.path()), after_first, "{flags:?}: the zip is byte for byte as it was");
    }

    // Blocks that were on from the start: no field (the phone reads it as on), an explicit true, and
    // each with a lock and a partner's key. None of them is given or rewritten an `on` field.
    let cases = [
        ("no field", on_with("a", Value::Null, None)),
        ("an explicit true", on_with("a", Value::Null, Some(true))),
        ("no field and a wait", on_with("a", wait(24), None)),
        ("a true and a key", on_with("a", with_partner_key(wait(24)), Some(true))),
    ];
    for (what, block) in cases {
        let folder = Scratch::new("switch-on-repeat-on");
        let file = write_zip(folder.path(), "s.zip", &[block.clone(), off_block("z")], &[]);
        let before = snapshot(folder.path());
        for flags in [&["--id", "a"][..], &["--id", "a", "--write"]] {
            let run = switch_on(&file, flags);
            assert_eq!((run.code, run.err.as_str(), run.out.as_str()), (0, "", ALREADY), "{what} {flags:?}");
            assert_eq!(snapshot(folder.path()), before, "{what} {flags:?}: byte for byte as it was, no file made");
        }
        assert_eq!(block_of(&file, "a").get("on"), block.get("on"), "{what}: the `on` field is as it was, absent or not");
        assert!(!waiting_of(&file).exists(), "{what}");
    }
}

#[test]
fn a_block_that_is_already_on_never_reaches_a_step_of_the_save() {
    for step in [Step::Write, Step::Flush, Step::Rename] {
        let folder = Scratch::new("switch-on-repeat-steps");
        let file = write_zip(folder.path(), "s.zip", &[on_with("a", Value::Null, None)], &[]);
        let before = snapshot(folder.path());
        let run = switch_on_with(&file, &["--id", "a", "--write"], &mut FailAt::new(step, 1));
        assert_eq!((run.code, run.err.as_str(), run.out.as_str()), (0, "", ALREADY), "{step:?}");
        assert_eq!(snapshot(folder.path()), before, "{step:?}");
    }
}
