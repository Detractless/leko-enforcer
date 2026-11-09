//! A settings file or a waiting file that cannot be used is reported in one plain line and left alone
//! (AC-009), for both commands, with and without `--write`: exit 1, nothing on stdout, both files and
//! the folder as they were. A bad waiting file is never replaced by a fresh one (that would lose the
//! parked changes), an empty file is not "nothing waiting", and a huge file is not read into memory.

mod parked_change_support;
use parked_change_support::*;

use delay_runs::{one_line, record, settle, switch_off, waiting_of, write_waiting, write_zip, NOW};
use delay_sets::{locked_block, zip_with};
use leko_cli::session::session_report::shown;
use leko_core::delayed_changes::waiting_file::read_waiting;
use leko_core::io::zip_reader::MAX_TOTAL_BYTES;
use run_capture::Outcome;
use scratch_folder::Scratch;
use serde_json::Value;
use std::path::Path;
use zip_writer::zip_of;

/// A file as a test sees it: its length, its modified time and (for a small file) its bytes.
type Seen = (String, u64, Option<std::time::SystemTime>, Option<Vec<u8>>);

/// Everything in `folder`; a big (sparse) file is judged by its length and time, not read.
fn seen(folder: &Path) -> Vec<Seen> {
    let mut all: Vec<Seen> = std::fs::read_dir(folder)
        .expect("readable")
        .map(|entry| {
            let entry = entry.expect("an entry");
            let facts = std::fs::symlink_metadata(entry.path()).expect("facts");
            let small = facts.is_file() && facts.len() < 2_000_000;
            (entry.file_name().to_string_lossy().into_owned(), facts.len(), facts.modified().ok(), small.then(|| std::fs::read(entry.path()).unwrap_or_default()))
        })
        .collect();
    all.sort();
    all
}

/// Runs each command (switch-off of block `a`, then settle), with and without `--write`: each must
/// refuse with the line `expected` and leave the folder exactly as it was.
fn assert_left_alone(folder: &Scratch, file: &Path, expected: &str, what: &str) {
    let before = seen(folder.path());
    for (command, write) in [("switch-off", false), ("switch-off", true), ("settle", false), ("settle", true)] {
        let mut flags: Vec<&str> = if command == "switch-off" { vec!["--id", "a"] } else { vec![] };
        flags.extend(write.then_some("--write"));
        let run: Outcome = if command == "switch-off" { switch_off(file, &flags, NOW) } else { settle(file, &flags, NOW) };
        let which = format!("{what}: {command}, write {write}");
        assert_eq!((run.code, run.out.as_str()), (1, ""), "{which}: {}", run.err);
        assert_eq!(one_line(&run.err), expected, "{which}");
        assert_eq!(seen(folder.path()), before, "{which}: no file was written, replaced, made or removed");
    }
}

#[test]
fn a_bad_settings_file_or_waiting_file_is_reported_and_nothing_is_overwritten() {
    let folder = Scratch::new("files");
    let path = |name: &str| folder.path().join(name);
    let named = |name: &str| shown(&path(name).to_string_lossy());
    let good = zip_with(&[locked_block("a", Value::Null)], &[]);

    // A settings file that is not there, or not a file.
    assert_left_alone(&folder, &path("missing.zip"), &nothing_saved(&format!("The settings file {} was not found.", named("missing.zip"))), "missing");
    std::fs::create_dir(path("folder.zip")).unwrap();
    assert_left_alone(&folder, &path("folder.zip"), &nothing_saved(&format!("{} is not a file.", named("folder.zip"))), "a folder");
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(path("nothing-here"), path("dangling.zip")).unwrap();
        assert_left_alone(&folder, &path("dangling.zip"), &nothing_saved(&format!("The settings file {} was not found.", named("dangling.zip"))), "a dangling link");
    }
    // Empty, not a zip, cut short, and zips the core refuses for what is inside.
    std::fs::write(path("empty.zip"), b"").unwrap();
    assert_left_alone(&folder, &path("empty.zip"), &nothing_saved(&format!("The settings file {} is empty.", named("empty.zip"))), "empty");
    let refused: [(&str, Vec<u8>); 4] = [
        ("text.zip", b"this is not a zip, it is a sentence\n".to_vec()),
        ("cut.zip", good[..60].to_vec()),
        ("bad-block.zip", zip_of(&[("GUIDE.md", "guide"), ("general.json", r#"{"schema": 2}"#), ("blocks/x.json", "{not json")])),
        ("no-settings.zip", zip_of(&[("GUIDE.md", "guide")])),
    ];
    for (name, bytes) in refused {
        std::fs::write(path(name), &bytes).unwrap();
        assert_left_alone(&folder, &path(name), &nothing_saved(&format!("The settings file {} was refused: {}", named(name), core_refusal(&bytes))), name);
    }
    // Over 64 MB is refused by its size, without reading it (a sparse 5 GB file is not read into memory).
    let sparse = |name: &str, length: u64| std::fs::File::create(path(name)).unwrap().set_len(length).unwrap();
    for (name, length) in [("one-over.zip", MAX_TOTAL_BYTES as u64 + 1), ("huge.zip", 5_000_000_000)] {
        sparse(name, length);
        assert_left_alone(&folder, &path(name), &nothing_saved(&format!("The settings file {} is larger than 64 MB, so it was not read.", named(name))), name);
    }
}

#[test]
fn a_bad_waiting_file_stops_every_change_and_is_never_replaced() {
    let folder = Scratch::new("waiting-files");
    let file = write_zip(folder.path(), "s.zip", &[locked_block("a", Value::Null)], &[]);
    let waiting = waiting_of(&file);
    let waiting_name = shown(&waiting.to_string_lossy());
    let line = |reason: &str| format!("The waiting file {waiting_name} {reason}.{LEFT_ALONE}{NOTHING_SAVED}");
    let one_record = |extra: &str| format!(r#"{{"format":1,"changes":[{{"kind":"switch-off","block":"b","requestedAt":"2026-11-02T14:00:00Z","dueAt":"2026-11-02T16:00:00Z"{extra}}}]}}"#);
    let by_the_core = |text: &str| read_waiting(text).expect_err("the core refuses this text");

    // Texts: the line is the core's reason after the file's name, and the file is not replaced.
    let texts: [(&str, String); 9] = [
        ("empty", String::new()),
        ("only spaces", "  \n".to_string()),
        ("not JSON", "{nope".to_string()),
        ("a list", "[]".to_string()),
        ("a newer format", r#"{"format":3,"changes":[]}"#.to_string()),
        ("no format", r#"{"changes":[]}"#.to_string()),
        ("a record without its moment", r#"{"format":1,"changes":[{"kind":"switch-off","block":"b"}]}"#.to_string()),
        ("a record with a field of its own", one_record(r#","note":"hi""#)),
        ("the same block twice", {
            let one = r#"{"kind":"switch-off","block":"b","requestedAt":"2026-11-02T14:00:00Z","dueAt":"2026-11-02T16:00:00Z"}"#;
            format!(r#"{{"format":1,"changes":[{one},{one}]}}"#)
        }),
    ];
    for (what, text) in texts {
        std::fs::write(&waiting, &text).unwrap();
        let reason = by_the_core(&text);
        let join = if reason.starts_with("change ") { "was refused: " } else { "" };
        assert_left_alone(&folder, &file, &line(&format!("{join}{reason}")), what);
    }
    std::fs::write(&waiting, r#"{"format":3,"changes":[]}"#).unwrap();
    assert_left_alone(&folder, &file, &line("is format 3, which is newer than the format 2 this program reads"), "format 3, in words");
    std::fs::write(&waiting, "").unwrap();
    assert_left_alone(&folder, &file, &line("is empty"), "an empty file is not 'nothing waiting'");

    // Bytes that are not text, a folder, a link to nowhere, and files over 1 MB (a sparse 5 GB one too).
    std::fs::write(&waiting, [0xff, 0xfe, 0x00]).unwrap();
    assert_left_alone(&folder, &file, &line("is not valid JSON"), "not text");
    let mut good_but_for_one_byte = serde_json::to_vec(&serde_json::json!({"format": 1, "changes": [record("zz", "2026-11-02T13:00:00Z", "2026-11-02T14:00:00Z")]})).unwrap();
    let at = good_but_for_one_byte.windows(2).position(|pair| pair == b"zz").unwrap();
    good_but_for_one_byte[at] = 0xff;
    std::fs::write(&waiting, &good_but_for_one_byte).unwrap();
    assert_left_alone(&folder, &file, &line("is not valid JSON"), "one byte that is not text inside a record that is good otherwise");
    std::fs::remove_file(&waiting).unwrap();
    std::fs::create_dir(&waiting).unwrap();
    assert_left_alone(&folder, &file, &format!("The waiting file {waiting_name} is not a file.{LEFT_ALONE}{NOTHING_SAVED}"), "a folder");
    std::fs::remove_dir(&waiting).unwrap();
    #[cfg(unix)]
    {
        let target = folder.path().join("nothing-here");
        std::os::unix::fs::symlink(&target, &waiting).unwrap();
        let before = seen(folder.path());
        for run in [switch_off(&file, &["--id", "a", "--write"], NOW), settle(&file, &["--write"], NOW)] {
            assert_eq!((run.code, run.out.as_str()), (1, ""), "{}", run.err);
            assert!(run.err.starts_with(&format!("The waiting file {waiting_name} could not be read (")), "{}", run.err);
            assert!(run.err.ends_with(&format!(").{LEFT_ALONE}{NOTHING_SAVED}")), "{}", run.err);
        }
        assert!(!target.exists(), "nothing was made through the link");
        assert_eq!(seen(folder.path()), before);
        std::fs::remove_file(&waiting).unwrap();
    }
    for length in [1_048_577, 5_000_000_000] {
        std::fs::File::create(&waiting).unwrap().set_len(length).unwrap();
        assert_left_alone(&folder, &file, &format!("The waiting file {waiting_name} is larger than 1 MB, so it was not read.{LEFT_ALONE}{NOTHING_SAVED}"), "too big");
    }
    std::fs::remove_file(&waiting).unwrap();

    // A waiting file nobody may read (skipped for the owner of all files, who can read everything).
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        write_waiting(&file, &[record("b", "2026-11-02T14:00:00Z", "2026-11-02T16:00:00Z")]);
        std::fs::set_permissions(&waiting, std::fs::Permissions::from_mode(0o000)).unwrap();
        if std::fs::File::open(&waiting).is_err() {
            for run in [switch_off(&file, &["--id", "a", "--write"], NOW), settle(&file, &["--write"], NOW)] {
                assert_eq!((run.code, run.out.as_str()), (1, ""));
                assert!(run.err.starts_with(&format!("The waiting file {waiting_name} could not be read (Permission denied")), "{}", run.err);
                assert_eq!(run.err.lines().count(), 1, "{}", run.err);
            }
        }
        std::fs::set_permissions(&waiting, std::fs::Permissions::from_mode(0o600)).unwrap();
        assert!(read_waiting(&std::fs::read_to_string(&waiting).unwrap()).is_ok(), "the parked change is still there");
    }
}

#[test]
fn a_full_waiting_file_takes_no_more_changes_and_a_longer_one_is_refused() {
    let folder = Scratch::new("waiting-full");
    let file = write_zip(folder.path(), "s.zip", &[locked_block("w", delay_sets::wait(24))], &[]);
    let waiting = waiting_of(&file);
    let waiting_name = shown(&waiting.to_string_lossy());
    let parked: Vec<Value> = (0..1000).map(|number| record(&format!("x{number}"), "2026-11-02T14:00:00Z", "2026-11-03T14:00:00Z")).collect();
    write_waiting(&file, &parked);
    let before = seen(folder.path());

    // A thousand changes are read; a thousand and first is not parked, and nothing is changed.
    let run = switch_off(&file, &["--id", "w", "--write"], NOW);
    assert_eq!((run.code, run.out.as_str()), (1, ""), "{}", run.err);
    assert_eq!(run.err, format!("The waiting file {waiting_name} would hold more than 1000 changes, so nothing was parked.\n"));
    assert_eq!(seen(folder.path()), before, "the full waiting file and the zip are as they were");
    let listed = settle(&file, &[], NOW);
    assert_eq!((listed.code, listed.err.as_str()), (0, ""));
    assert_eq!(listed.out.lines().count(), 1001, "a thousand changes, each its own line, and the closing line");

    // A file that holds a thousand and one is refused whole.
    let mut too_many = parked;
    too_many.push(record("x1000", "2026-11-02T14:00:00Z", "2026-11-03T14:00:00Z"));
    write_waiting(&file, &too_many);
    let before = seen(folder.path());
    assert_left_alone(&folder, &file, &line_of(&waiting_name, "holds more than 1000 changes"), "a thousand and one changes");
    assert_eq!(seen(folder.path()), before);
}

fn line_of(waiting_name: &str, reason: &str) -> String {
    format!("The waiting file {waiting_name} {reason}.{LEFT_ALONE}{NOTHING_SAVED}")
}
