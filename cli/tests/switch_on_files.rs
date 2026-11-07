//! A settings file or a waiting file that cannot be used stops `leko switch-on` (AC-010): exit 1, one
//! plain sentence ending "Nothing was saved." (or, for the waiting file, saying it was left as it was),
//! nothing on stdout, and neither file replaced, repaired or made. A waiting file that is not there is no
//! problem. A bad waiting file is never read as "nothing parked", since that would miss a switch-off that
//! is still waiting; with and without `--write`.

mod delay_runs;
#[path = "../../core/tests/delay_sets/mod.rs"]
mod delay_sets;
mod run_capture;
mod scratch_folder;
#[path = "../../core/tests/zip_writer/mod.rs"]
mod zip_writer;

use delay_runs::{one_line, snapshot, switch_on, waiting_of, write_zip};
use delay_sets::{off_block, zip_with};
use leko_cli::session::session_report::shown;
use leko_core::io::zip_check::{check_zip, ZipVerdict};
use leko_core::io::zip_reader::MAX_TOTAL_BYTES;
use scratch_folder::Scratch;
use serde_json::json;
use std::collections::HashSet;
use std::path::Path;
use zip_writer::zip_of;

const NOTHING_SAVED: &str = " Nothing was saved.\n";
const LEFT_ALONE: &str = " It was left as it was.";
const GOOD: &str = r#"{"kind":"switch-off","block":"a","requestedAt":"2026-11-02T14:00:00Z","dueAt":"2026-11-03T14:00:00Z"}"#;

/// Runs a switch-on of `a`, with and without `--write`: each must refuse with `expected` and leave
/// the folder exactly as it was.
fn assert_left_alone(folder: &Scratch, file: &Path, expected: &str, what: &str) {
    let before = snapshot(folder.path());
    for flags in [&["--id", "a"][..], &["--id", "a", "--write"]] {
        let run = switch_on(file, flags);
        assert_eq!((run.code, run.out.as_str()), (1, ""), "{what} {flags:?}: {}", run.err);
        assert_eq!(one_line(&run.err), expected, "{what} {flags:?}");
        assert_eq!(snapshot(folder.path()), before, "{what} {flags:?}: no file was written, replaced, made or removed");
    }
}

fn core_refusal(bytes: &[u8]) -> String {
    match check_zip(bytes, &HashSet::new()) {
        ZipVerdict::Refused(refusal) => refusal.sentence,
        ZipVerdict::Read(_) => panic!("the core was expected to refuse these {} bytes", bytes.len()),
    }
}

fn nothing_saved(sentence: &str) -> String {
    let stop = if sentence.ends_with(['.', '!', '?']) { "" } else { "." };
    format!("{sentence}{stop}{NOTHING_SAVED}")
}

#[test]
fn a_bad_settings_or_waiting_file_is_reported_and_left_alone() {
    // A settings file that is missing, a folder, empty, not a zip, cut short, or refused by the core.
    let folder = Scratch::new("switch-on-files");
    let path = |name: &str| folder.path().join(name);
    let named = |name: &str| shown(&path(name).to_string_lossy());
    let good = zip_with(&[off_block("a")], &[]);
    assert_left_alone(&folder, &path("missing.zip"), &nothing_saved(&format!("The settings file {} was not found.", named("missing.zip"))), "missing");
    std::fs::create_dir(path("folder.zip")).unwrap();
    assert_left_alone(&folder, &path("folder.zip"), &nothing_saved(&format!("{} is not a file.", named("folder.zip"))), "a folder");
    std::fs::write(path("empty.zip"), b"").unwrap();
    assert_left_alone(&folder, &path("empty.zip"), &nothing_saved(&format!("The settings file {} is empty.", named("empty.zip"))), "empty");
    let mut odd_on = off_block("a");
    odd_on["on"] = json!("yes");
    let refused: [(&str, Vec<u8>); 4] = [
        ("text.zip", b"this is not a zip, it is a sentence\n".to_vec()),
        ("cut.zip", good[..60].to_vec()),
        ("bad-block.zip", zip_of(&[("GUIDE.md", "guide"), ("general.json", r#"{"schema": 2}"#), ("blocks/a.json", "{not json")])),
        ("odd-on.zip", zip_with(&[odd_on], &[])),
    ];
    for (name, bytes) in refused {
        std::fs::write(path(name), &bytes).unwrap();
        assert_left_alone(&folder, &path(name), &nothing_saved(&format!("The settings file {} was refused: {}", named(name), core_refusal(&bytes))), name);
    }
    // Over 64 MB is refused by its size, without reading it (a sparse file is not read into memory).
    std::fs::File::create(path("big.zip")).unwrap().set_len(MAX_TOTAL_BYTES as u64 + 1).unwrap();
    let before = std::fs::metadata(path("big.zip")).unwrap();
    for flags in [&["--id", "a"][..], &["--id", "a", "--write"]] {
        let run = switch_on(&path("big.zip"), flags);
        assert_eq!((run.code, run.out.as_str()), (1, ""), "{flags:?}");
        assert_eq!(one_line(&run.err), nothing_saved(&format!("The settings file {} is larger than 64 MB, so it was not read.", named("big.zip"))), "{flags:?}");
    }
    let after = std::fs::metadata(path("big.zip")).unwrap();
    assert_eq!((before.len(), before.modified().unwrap()), (after.len(), after.modified().unwrap()), "the big file was not touched");
}

#[test]
fn a_bad_waiting_file_stops_a_switch_on_and_is_never_replaced() {
    let folder = Scratch::new("switch-on-waiting-files");
    let file = write_zip(folder.path(), "s.zip", &[off_block("a")], &[]);
    let waiting = waiting_of(&file);
    let waiting_name = shown(&waiting.to_string_lossy());
    let line = |reason: &str| format!("The waiting file {waiting_name} {reason}.{LEFT_ALONE}{NOTHING_SAVED}");
    let mut big = br#"{"format":1,"changes":[]}"#.to_vec();
    big.resize(1_048_577, b' ');
    let cases: Vec<(&str, Vec<u8>, String)> = vec![
        ("empty", Vec::new(), line("is empty")),
        ("only spaces", b"  \n".to_vec(), line("is empty")),
        ("not JSON", b"{nope".to_vec(), line("is not valid JSON")),
        ("not text", vec![0xFF, 0xFE, 0x00, 0x41], line("is not valid JSON")),
        ("a newer format", br#"{"format":3,"changes":[]}"#.to_vec(), line("is format 3, which is newer than the format 2 this program reads")),
        ("a record the reader cannot read", br#"{"format":1,"changes":[{"kind":"switch-off","block":"b"}]}"#.to_vec(), String::new()),
        ("a kind it does not know", format!(r#"{{"format":1,"changes":[{{"kind":"switch-on","block":"a","requestedAt":"2026-11-02T14:00:00Z","dueAt":"2026-11-03T14:00:00Z"}}]}}"#).into_bytes(), String::new()),
        ("the same block twice", format!(r#"{{"format":1,"changes":[{GOOD},{GOOD}]}}"#).into_bytes(), String::new()),
        ("over 1 MB", big, line("is larger than 1 MB, so it was not read")),
    ];
    for (what, bytes, expected) in cases {
        std::fs::write(&waiting, &bytes).unwrap();
        let before = snapshot(folder.path());
        for flags in [&["--id", "a"][..], &["--id", "a", "--write"]] {
            let run = switch_on(&file, flags);
            assert_eq!((run.code, run.out.as_str()), (1, ""), "{what} {flags:?}: {}", run.err);
            let sentence = one_line(&run.err);
            assert!(sentence.starts_with(&format!("The waiting file {waiting_name} ")) && sentence.ends_with(&format!("{LEFT_ALONE}{NOTHING_SAVED}")), "{what} {flags:?}: {sentence}");
            if !expected.is_empty() {
                assert_eq!(sentence, expected, "{what} {flags:?}");
            }
            assert!(!sentence.contains("panicked") && !sentence.contains("RUST_BACKTRACE"), "{what}: a sentence, not a stack trace");
            assert_eq!(snapshot(folder.path()), before, "{what} {flags:?}: neither file was replaced, repaired or made");
        }
    }

    // A folder where the waiting file should be.
    std::fs::remove_file(&waiting).unwrap();
    std::fs::create_dir(&waiting).unwrap();
    std::fs::write(waiting.join("inside.txt"), b"mine").unwrap();
    assert_left_alone(&folder, &file, &line("is not a file"), "a folder");
    assert!(waiting.join("inside.txt").is_file());

    // No waiting file at all is not a problem, and none is made.
    std::fs::remove_dir_all(&waiting).unwrap();
    let run = switch_on(&file, &["--id", "a", "--write"]);
    assert_eq!((run.code, run.err.as_str()), (0, ""), "{}", run.out);
    assert!(!waiting.exists(), "a switch-on never makes a waiting file");
}
