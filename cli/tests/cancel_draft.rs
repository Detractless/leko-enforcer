//! `leko cancel FILE --id ID` without `--write` is a draft (AC-003): one plain sentence naming the
//! block, the words that nothing was saved, and the same command with `--write`. The settings zip, the
//! waiting file and the folder are as they were, and so are the program's home and temporary folders.

mod parked_change_support;
use parked_change_support::{delay_runs, delay_sets, run_capture, scratch_folder, zip_writer};

use delay_runs::{cancel, record, snapshot, write_waiting, write_zip};
use delay_sets::{locked_block, wait};
use leko_cli::session::session_report::quoted;
use scratch_folder::{run_leko, Scratch};

fn blocks() -> Vec<serde_json::Value> {
    vec![locked_block("a", wait(24)), locked_block("b", wait(24))]
}

fn parked(file: &std::path::Path) {
    write_waiting(file, &[record("a", "2026-11-02T14:00:00Z", "2026-11-03T14:00:00Z"), record("b", "2026-11-02T13:00:00Z", "2026-11-03T13:00:00Z")]);
}

#[test]
fn without_the_write_flag_the_draft_says_what_would_be_cancelled_and_saves_nothing() {
    for name in ["s.zip", "my settings.zip"] {
        let folder = Scratch::new("cancel-draft");
        let file = write_zip(folder.path(), name, &blocks(), &[]);
        parked(&file);
        let before = snapshot(folder.path());

        let run = cancel(&file, &["--id", "a"]);
        let command = format!("leko cancel {} --id a", quoted(&file.to_string_lossy()));
        assert_eq!((run.code, run.err.as_str()), (0, ""), "{name}");
        assert_eq!(run.out, format!("Draft: the waiting change for the block \"a\" would be cancelled. Nothing was saved.\nTo cancel it, run: {command} --write\n"), "{name}");
        assert_eq!(snapshot(folder.path()), before, "{name}: the zip, the waiting file and the folder are as they were, no temporary file");

        // The block is named as it was typed, and the other block's record is not in the sentence.
        let other = cancel(&file, &["--id", "b"]);
        assert!(other.out.contains("the block \"b\" would be cancelled") && !other.out.contains("\"a\""), "{}", other.out);
        assert_eq!(snapshot(folder.path()), before, "{name}: still as it was");
    }
    // A name with a space is written so that pasting the command into a shell gives the same file.
    let folder = Scratch::new("cancel-draft-quoted");
    let file = write_zip(folder.path(), "my settings.zip", &blocks(), &[]);
    parked(&file);
    assert!(cancel(&file, &["--id", "a"]).out.contains("run: leko cancel '"), "a path with a space is quoted");
}

#[test]
fn the_real_program_leaves_its_home_and_temporary_folders_empty_when_it_drafts_a_cancel() {
    let folder = Scratch::new("cancel-draft-real");
    let (home, temp) = (Scratch::new("cancel-draft-home"), Scratch::new("cancel-draft-temp"));
    let file = write_zip(folder.path(), "s.zip", &blocks(), &[]);
    parked(&file);
    let before = snapshot(folder.path());
    let run = run_leko(&home, &temp, &["cancel", file.to_str().expect("text path"), "--id", "a"]);
    assert_eq!(run.status.code(), Some(0), "{}", String::from_utf8_lossy(&run.stderr));
    assert!(String::from_utf8_lossy(&run.stdout).contains("Nothing was saved"));
    assert!(run.stderr.is_empty());
    assert_eq!((home.names(), temp.names(), snapshot(folder.path())), (vec![], vec![], before));
}
