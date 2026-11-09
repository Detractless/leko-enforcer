//! `leko switch-on FILE --id ID` without `--write` is a draft (AC-003): one plain sentence naming the
//! block, the words that nothing was saved, and the same command with `--write`. The settings zip, the
//! waiting file and the folder are as they were, no waiting file is made, and so are the program's home
//! and temporary folders.

mod parked_change_support;
use parked_change_support::*;

use delay_runs::{record, snapshot, switch_on, waiting_of, write_waiting, write_zip};
use delay_sets::{locked_block, off_block, wait};
use leko_cli::session::session_report::quoted;
use scratch_folder::{run_leko, Scratch};
use serde_json::{json, Value};

fn blocks() -> Vec<Value> {
    let mut locked = locked_block("b", wait(24));
    locked["on"] = json!(false);
    vec![off_block("a"), locked]
}

#[test]
fn without_the_write_flag_the_draft_says_what_would_happen_and_saves_nothing() {
    for name in ["s.zip", "my settings.zip"] {
        let folder = Scratch::new("switch-on-draft");
        let file = write_zip(folder.path(), name, &blocks(), &[]);
        let before = snapshot(folder.path());

        let run = switch_on(&file, &["--id", "a"]);
        let command = format!("leko switch-on {} --id a", quoted(&file.to_string_lossy()));
        assert_eq!((run.code, run.err.as_str()), (0, ""), "{name}");
        assert_eq!(run.out, format!("Draft: the block \"a\" would be switched on now. Nothing was saved.\nTo switch it on, run: {command} --write\n"), "{name}");
        assert_eq!(snapshot(folder.path()), before, "{name}: the zip and the folder are as they were, no waiting file and no temporary file");
        assert!(!waiting_of(&file).exists(), "{name}: a draft makes no waiting file");

        // A block with a lock is drafted the same way: switching on is never held, and is named as typed.
        let locked = switch_on(&file, &["--id", "b"]);
        assert_eq!((locked.code, locked.err.as_str()), (0, ""), "{name}");
        assert!(locked.out.starts_with("Draft: the block \"b\" would be switched on now. Nothing was saved.\n") && !locked.out.contains("\"a\""), "{}", locked.out);
        assert_eq!(snapshot(folder.path()), before, "{name}: still as it was");
    }

    // The words come back in the order they were typed, and a name with a space is written so that
    // pasting the command into a shell gives the same file.
    let folder = Scratch::new("switch-on-draft-order");
    let file = write_zip(folder.path(), "my settings.zip", &blocks(), &[]);
    let run = switch_on(&file, &["--id", "a", "--write", "--write"]);
    assert_eq!(run.code, 2, "a bad request is not drafted: {}", run.out);
    let run = switch_on(&file, &["--id", "a"]);
    assert!(run.out.contains("run: leko switch-on '") && run.out.ends_with(" --id a --write\n"), "{}", run.out);
}

#[test]
fn a_draft_leaves_a_waiting_file_beside_the_zip_as_it_was() {
    let folder = Scratch::new("switch-on-draft-waiting");
    let file = write_zip(folder.path(), "s.zip", &blocks(), &[]);
    write_waiting(&file, &[record("b", "2026-11-02T14:00:00Z", "2026-11-03T14:00:00Z")]);
    let before = snapshot(folder.path());
    // The record is for another block: it does not change what is said about `a`.
    let run = switch_on(&file, &["--id", "a"]);
    assert_eq!((run.code, run.err.as_str()), (0, ""));
    assert!(run.out.starts_with("Draft: the block \"a\" would be switched on now."), "{}", run.out);
    assert_eq!(snapshot(folder.path()), before);
}

#[test]
fn the_real_program_leaves_its_home_and_temporary_folders_empty_when_it_drafts_a_switch_on() {
    let folder = Scratch::new("switch-on-draft-real");
    let (home, temp) = (Scratch::new("switch-on-draft-home"), Scratch::new("switch-on-draft-temp"));
    let file = write_zip(folder.path(), "s.zip", &blocks(), &[]);
    let before = snapshot(folder.path());
    let run = run_leko(&home, &temp, &["switch-on", file.to_str().expect("text path"), "--id", "a"]);
    assert_eq!(run.status.code(), Some(0), "{}", String::from_utf8_lossy(&run.stderr));
    assert!(String::from_utf8_lossy(&run.stdout).contains("Nothing was saved"));
    assert!(run.stderr.is_empty());
    assert_eq!((home.names(), temp.names(), snapshot(folder.path())), (vec![], vec![], before));
}
