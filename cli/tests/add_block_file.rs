//! A file that cannot be added to is reported plainly and left alone (AC-010): a missing path, a
//! folder, an unreadable file, an empty file, a file that is not a zip, a zip the core refuses (its
//! sentence is shown and nothing is repaired) and a file over 64 MB each exit 1 with one line on
//! stderr, nothing on stdout, and the file and its folder unchanged, with and without `--write`.

mod add_block_runs;
mod run_capture;
mod scratch_folder;
mod settings_builder;

use add_block_runs::{add, plain};
use leko_cli::session::session_report::shown;
use leko_core::io::zip_check::{check_zip, ZipVerdict};
use leko_core::io::zip_reader::MAX_TOTAL_BYTES;
use scratch_folder::Scratch;
use serde_json::json;
use settings_builder::{always, block, settings_bytes, zip_of};
use std::collections::HashSet;
use std::path::Path;
use std::time::SystemTime;

/// The core's sentence for refusing these bytes.
fn core_refusal(bytes: &[u8]) -> String {
    match check_zip(bytes, &HashSet::new()) {
        ZipVerdict::Refused(refusal) => refusal.sentence,
        ZipVerdict::Read(_) => panic!("the core was expected to refuse these {} bytes", bytes.len()),
    }
}

/// `sentence` and " Nothing was saved.", with a full stop between them when the sentence has none.
fn with_nothing_saved(sentence: &str) -> String {
    let stop = if sentence.ends_with(['.', '!', '?']) { "" } else { "." };
    format!("{sentence}{stop} Nothing was saved.\n")
}

/// The bytes of `path` when it is a small file (a sparse 5 GB file is not read into memory here).
fn small_bytes(path: &Path) -> Option<Vec<u8>> {
    let small = std::fs::metadata(path).map(|facts| facts.is_file() && facts.len() < 1_000_000).unwrap_or(false);
    small.then(|| std::fs::read(path).ok()).flatten()
}

fn mtime(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).ok().and_then(|facts| facts.modified().ok())
}

/// Runs the plain request on `file` with and without `--write`; each run must fail with `expected`
/// and leave the file (its bytes and its modified time, if there is one) and the folder alone.
fn assert_left_alone(folder: &Scratch, file: &Path, expected: &str, what: &str) {
    let (names, bytes, size, modified) = (folder.names(), small_bytes(file), std::fs::metadata(file).ok().map(|facts| facts.len()), mtime(file));
    for write in [false, true] {
        let flags = [plain(), if write { vec!["--write"] } else { vec![] }].concat();
        let run = add(file, &flags);
        assert_eq!((run.code, run.out.as_str()), (1, ""), "{what}, write {write}: {}", run.err);
        assert_eq!(run.err, expected, "{what}, write {write}");
        assert_eq!(run.err.lines().count(), 1, "{what}: one line");
        assert_eq!(small_bytes(file), bytes, "{what}, write {write}: the file is as it was");
        assert_eq!(std::fs::metadata(file).ok().map(|facts| facts.len()), size, "{what}, write {write}: same size");
        assert_eq!(mtime(file), modified, "{what}, write {write}: the file was not touched");
        assert_eq!(folder.names(), names, "{what}, write {write}: no file came or went");
    }
}

#[test]
fn a_file_that_cannot_be_added_to_is_reported_and_left_untouched() {
    let folder = Scratch::new("file");
    let path = |name: &str| folder.path().join(name);
    let named = |file: &Path| shown(&file.to_string_lossy());
    let good = settings_bytes(&[block("b", json!({"apps": ["com.a"]}), json!([always("a1", &[])]))]);

    // Not there, or not a file.
    assert_left_alone(&folder, &path("missing.zip"), &with_nothing_saved(&format!("The settings file {} was not found.", named(&path("missing.zip")))), "missing");
    std::fs::create_dir(path("folder.zip")).unwrap();
    std::fs::write(path("folder.zip").join("inside.txt"), b"x").unwrap();
    assert_left_alone(&folder, &path("folder.zip"), &with_nothing_saved(&format!("{} is not a file.", named(&path("folder.zip")))), "a folder");
    assert_eq!(std::fs::read(path("folder.zip").join("inside.txt")).unwrap(), b"x");
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(path("nothing-here"), path("dangling.zip")).unwrap();
        assert_left_alone(&folder, &path("dangling.zip"), &with_nothing_saved(&format!("The settings file {} was not found.", named(&path("dangling.zip")))), "a dangling link");
    }

    // Empty, not a zip, cut short, and zips the core refuses for what is inside: its sentence is shown.
    std::fs::write(path("empty.zip"), b"").unwrap();
    assert_left_alone(&folder, &path("empty.zip"), &with_nothing_saved(&format!("The settings file {} is empty.", named(&path("empty.zip")))), "empty");
    let bad_block = zip_of(&[("GUIDE.md", "guide"), ("general.json", r#"{"schema": 2}"#), ("blocks/x.json", "{not json")]);
    let refused: [(&str, Vec<u8>); 5] = [
        ("text.zip", b"this is not a zip, it is a sentence\n".to_vec()),
        ("one-byte.zip", b"P".to_vec()),
        ("cut.zip", good[..60].to_vec()),
        ("bad-block.zip", bad_block),
        ("no-settings.zip", zip_of(&[("GUIDE.md", "guide")])),
    ];
    for (name, bytes) in refused {
        std::fs::write(path(name), &bytes).unwrap();
        assert_left_alone(&folder, &path(name), &with_nothing_saved(&format!("The settings file was refused: {}", core_refusal(&bytes))), name);
    }

    // A file the program may not read (skipped for the owner of all files, who can read everything).
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::write(path("locked.zip"), &good).unwrap();
        std::fs::set_permissions(path("locked.zip"), std::fs::Permissions::from_mode(0o000)).unwrap();
        if std::fs::File::open(path("locked.zip")).is_err() {
            let start = format!("The settings file {} could not be read (Permission denied", named(&path("locked.zip")));
            for write in [false, true] {
                let run = add(&path("locked.zip"), &[plain(), if write { vec!["--write"] } else { vec![] }].concat());
                assert_eq!((run.code, run.out.as_str()), (1, ""));
                assert!(run.err.starts_with(&start) && run.err.ends_with(". Nothing was saved.\n") && run.err.lines().count() == 1, "{}", run.err);
            }
        }
        std::fs::set_permissions(path("locked.zip"), std::fs::Permissions::from_mode(0o600)).unwrap();
    }
}

#[test]
fn a_file_over_64_mb_is_refused_by_its_size_and_exactly_64_mb_is_read_and_refused_by_the_core() {
    let folder = Scratch::new("big");
    let path = |name: &str| folder.path().join(name);
    let sparse = |name: &str, length: u64| std::fs::File::create(path(name)).unwrap().set_len(length).unwrap();
    let over = |name: &str| with_nothing_saved(&format!("The settings file {} is larger than 64 MB, so it was not read.", shown(&path(name).to_string_lossy())));
    let limit = MAX_TOTAL_BYTES as u64;

    sparse("one-over.zip", limit + 1);
    assert_left_alone(&folder, &path("one-over.zip"), &over("one-over.zip"), "one byte over");
    sparse("huge.zip", 5_000_000_000);
    assert_left_alone(&folder, &path("huge.zip"), &over("huge.zip"), "5 GB");
    // The size decides before the file is opened: a file nobody may read still gets the size sentence.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        sparse("locked-big.zip", limit + 1);
        std::fs::set_permissions(path("locked-big.zip"), std::fs::Permissions::from_mode(0o000)).unwrap();
        assert_left_alone(&folder, &path("locked-big.zip"), &over("locked-big.zip"), "too big and locked");
        std::fs::set_permissions(path("locked-big.zip"), std::fs::Permissions::from_mode(0o600)).unwrap();
    }
    sparse("exact.zip", limit);
    for write in [false, true] {
        let run = add(&path("exact.zip"), &[plain(), if write { vec!["--write"] } else { vec![] }].concat());
        assert_eq!((run.code, run.out.as_str()), (1, ""));
        assert!(run.err.contains("was refused:") && !run.err.contains("larger than 64 MB"), "{}", run.err);
    }
    assert_eq!(std::fs::metadata(path("exact.zip")).unwrap().len(), limit, "the file was not changed");
}
