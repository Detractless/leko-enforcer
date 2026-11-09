//! A waiting file that cannot be read stops `leko cancel` (AC-008): one plain sentence on stderr, exit
//! 1, nothing on stdout, and the file is never replaced or repaired. This holds for an empty file, one
//! that is not JSON, not text, of a newer format, with a record that cannot be read, a folder, and
//! one over 1 MB; with and without `--write`.

mod parked_change_support;
use parked_change_support::{delay_runs, delay_sets, run_capture, scratch_folder, zip_writer};

use delay_runs::{cancel, one_line, snapshot, waiting_of, write_zip};
use delay_sets::{locked_block, wait};
use leko_cli::session::session_report::shown;
use scratch_folder::Scratch;

const GOOD: &str = r#"{"kind":"switch-off","block":"a","requestedAt":"2026-11-02T14:00:00Z","dueAt":"2026-11-03T14:00:00Z"}"#;

/// What the waiting file holds in each case, and the words the sentence must carry.
fn bad_files() -> Vec<(&'static str, Vec<u8>, &'static str)> {
    let mut big = br#"{"format":1,"changes":[]}"#.to_vec();
    big.resize(1_048_577, b' ');
    vec![
        ("empty", Vec::new(), "is empty"),
        ("not JSON", b"hello there".to_vec(), "is not valid JSON"),
        ("not text", vec![0xFF, 0xFE, 0x00, 0x41], "is not valid JSON"),
        ("a newer format", format!(r#"{{"format":4,"changes":[{GOOD}]}}"#).into_bytes(), "is format 4, which is newer than the format 3 this program reads"),
        ("a newer format with data of its own", format!(r#"{{"format":2,"changes":[{GOOD}],"extra":{{"keep":[1,2,3]}}}}"#).into_bytes(), "has a field \"extra\" that this file does not have"),
        ("a record the reader cannot read", format!(r#"{{"format":1,"changes":[{GOOD},{{"kind":"switch-off","block":"B!","requestedAt":"x","dueAt":"y"}}]}}"#).into_bytes(), "change 2: requestedAt must be a UTC moment"),
        ("a record of a kind it does not know", format!(r#"{{"format":1,"changes":[{GOOD},{{"kind":"switch-on","block":"b","requestedAt":"2026-11-02T14:00:00Z","dueAt":"2026-11-03T14:00:00Z"}}]}}"#).into_bytes(), "change 2 has a kind \"switch-on\" that this program does not know"),
        ("over 1 MB", big, "larger than 1 MB"),
    ]
}

#[test]
fn a_bad_waiting_file_is_reported_and_left_alone_by_cancel() {
    for (what, bytes, word) in bad_files() {
        let folder = Scratch::new("cancel-files");
        let file = write_zip(folder.path(), "s.zip", &[locked_block("a", wait(24))], &[]);
        std::fs::write(waiting_of(&file), &bytes).unwrap();
        let before = snapshot(folder.path());
        let start = format!("The waiting file {}", shown(&waiting_of(&file).to_string_lossy()));
        for flags in [&["--id", "a"][..], &["--id", "a", "--write"], &["--id", "zzz", "--write"]] {
            let run = cancel(&file, flags);
            assert_eq!((run.code, run.out.as_str()), (1, ""), "{what} {flags:?}: {}", run.err);
            let sentence = one_line(&run.err);
            assert!(sentence.starts_with(&start) && sentence.contains(word), "{what}: {sentence}");
            assert!(sentence.ends_with("It was left as it was. Nothing was saved.\n"), "{what}: {sentence}");
            assert!(!sentence.contains("panicked") && !sentence.contains("RUST_BACKTRACE"), "{what}: a sentence, not a stack trace");
            assert_eq!(snapshot(folder.path()), before, "{what} {flags:?}: never replaced, repaired or rewritten");
        }
        assert_eq!(std::fs::read(waiting_of(&file)).unwrap(), bytes, "{what}: byte for byte as it was");
    }
}

#[test]
fn a_folder_where_the_waiting_file_should_be_is_reported_and_left_alone() {
    let folder = Scratch::new("cancel-files-dir");
    let file = write_zip(folder.path(), "s.zip", &[locked_block("a", wait(24))], &[]);
    std::fs::create_dir(waiting_of(&file)).unwrap();
    std::fs::write(waiting_of(&file).join("inside.txt"), b"mine").unwrap();
    for flags in [&["--id", "a"][..], &["--id", "a", "--write"]] {
        let run = cancel(&file, flags);
        assert_eq!((run.code, run.out.as_str()), (1, ""), "{flags:?}: {}", run.err);
        assert!(one_line(&run.err).starts_with("The waiting file ") && run.err.contains("is not a file"), "{}", run.err);
        assert!(run.err.ends_with("It was left as it was. Nothing was saved.\n"), "{}", run.err);
    }
    assert!(waiting_of(&file).is_dir() && waiting_of(&file).join("inside.txt").is_file(), "the folder and what is in it are as they were");
}
