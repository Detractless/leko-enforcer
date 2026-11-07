//! A settings file that cannot be read, or that the core refuses, gives no answer (AC-009): exit 1, one
//! plain sentence on stderr that names the file safely, nothing on stdout (with `--json` too), no
//! usage. A file over 64 MB is refused by its size before it is opened. A zip with no blocks is not a
//! failure: it is a settings file that closes nothing.

mod run_capture;
mod scratch_folder;
mod settings_builder;

use leko_cli::session::session_report::shown;
use leko_core::io::zip_check::{check_zip, ZipVerdict};
use leko_core::io::zip_reader::MAX_TOTAL_BYTES;
use run_capture::{status, Outcome};
use scratch_folder::Scratch;
use serde_json::json;
use settings_builder::{always, block, settings_bytes, write_settings, zip_of};
use std::collections::HashSet;
use std::path::Path;

const NOW: &str = "2026-11-05T15:00:00Z";

/// The command run twice, without and with `--json`: both must give the same failure.
fn asked(file: &Path) -> Vec<Outcome> {
    [vec!["--app", "com.a", "--zone", "UTC"], vec!["--json", "--app", "com.a", "--zone", "UTC"]].iter().map(|flags| status(file, flags, NOW, None)).collect()
}

fn no_answer(file: &Path, sentence: &str) {
    for run in asked(file) {
        assert_eq!((run.code, run.out.as_str()), (1, ""), "{sentence}: {}", run.err);
        assert_eq!(run.err, format!("{sentence}\n"), "one sentence, and no usage");
        assert!(!run.err.contains('\u{1b}'), "a control character reached the terminal");
    }
}

/// The sentence the core gives for refusing these bytes.
fn core_refusal(bytes: &[u8]) -> String {
    match check_zip(bytes, &HashSet::new()) {
        ZipVerdict::Refused(refusal) => refusal.sentence,
        ZipVerdict::Read(_) => panic!("the core was expected to refuse these {} bytes", bytes.len()),
    }
}

fn refused(file: &Path, bytes: &[u8]) {
    std::fs::write(file, bytes).expect("written");
    no_answer(file, &format!("The settings file {} was refused: {}", shown(&file.to_string_lossy()), core_refusal(bytes)));
}

#[test]
fn a_file_that_cannot_be_read_or_is_refused_gives_no_answer_and_exit_1() {
    let folder = Scratch::new("file");
    let path = |name: &str| folder.path().join(name);
    let named = |file: &Path| shown(&file.to_string_lossy());

    // Not there, or not a file.
    no_answer(&path("missing.zip"), &format!("The settings file {} was not found.", named(&path("missing.zip"))));
    std::fs::create_dir(path("folder.zip")).expect("a folder");
    no_answer(&path("folder.zip"), &format!("{} is not a file.", named(&path("folder.zip"))));
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(path("nothing-here"), path("dangling.zip")).expect("a link");
        no_answer(&path("dangling.zip"), &format!("The settings file {} was not found.", named(&path("dangling.zip"))));
    }

    // Empty, not a zip, cut short, and a zip the core refuses for what is inside.
    std::fs::write(path("empty.zip"), b"").expect("written");
    no_answer(&path("empty.zip"), &format!("The settings file {} is empty.", named(&path("empty.zip"))));
    refused(&path("text.zip"), b"this is not a zip, it is a sentence\n");
    refused(&path("one-byte.zip"), b"P");
    let good = settings_bytes(&[block("b", json!({"apps": ["com.a"]}), json!([always("a1", &[])]))]);
    refused(&path("cut.zip"), &good[..60]);
    // The core reads a zip the way the phone does, from the start and up to the central directory, so a
    // zip that lost only the end of that directory is still read, and answered like the whole one.
    std::fs::write(path("whole.zip"), &good).expect("written");
    std::fs::write(path("cut-end.zip"), &good[..good.len() - 10]).expect("written");
    let (whole, cut_end) = (status(&path("whole.zip"), &["--app", "com.a", "--zone", "UTC"], NOW, None), status(&path("cut-end.zip"), &["--app", "com.a", "--zone", "UTC"], NOW, None));
    assert_eq!((cut_end.code, cut_end.err.as_str()), (0, ""), "{}", cut_end.err);
    assert_eq!((whole.code, whole.out.as_str()), (0, cut_end.out.as_str()));
    refused(&path("bad-block.zip"), &zip_of(&[("GUIDE.md", "guide"), ("general.json", r#"{"schema": 2}"#), ("blocks/x.json", "{not json")]));
    refused(&path("no-settings.zip"), &zip_of(&[("GUIDE.md", "guide")]));

    // A file the program may not read: the sentence says it could not be read (skipped as the owner of
    // all files, who can read everything).
    std::fs::write(path("locked.zip"), &good).expect("written");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path("locked.zip"), std::fs::Permissions::from_mode(0o000)).expect("permissions");
        if std::fs::File::open(path("locked.zip")).is_err() {
            for run in asked(&path("locked.zip")) {
                assert_eq!((run.code, run.out.as_str()), (1, ""));
                let start = format!("The settings file {} could not be read (Permission denied", named(&path("locked.zip")));
                assert!(run.err.starts_with(&start) && run.err.matches('\n').count() == 1, "{}", run.err);
            }
        }
    }

    // A file name that would steer or flood a terminal is written escaped and cut short.
    // (a relative name that is not there, so the whole name fits in what is written)
    let odd = Path::new("a\u{1b}[31mred\n\u{7f}.zip");
    no_answer(odd, &format!("The settings file {} was not found.", named(odd)));
    assert!(named(odd).contains("\\u{1b}") && !named(odd).contains('\u{1b}') && !named(odd).contains('\n'), "{}", named(odd));
    let odd_in_folder = path("a\u{1b}[31mred\n\u{7f}.zip");
    no_answer(&odd_in_folder, &format!("The settings file {} was not found.", named(&odd_in_folder)));
    let long = folder.path().join("y".repeat(200));
    let run = status(&long, &["--app", "com.a", "--zone", "UTC"], NOW, None);
    assert_eq!((run.code, run.out.as_str(), run.err.matches('\n').count()), (1, "", 1));
    assert!(run.err.contains("...") && run.err.len() < 120, "{} bytes: {}", run.err.len(), run.err);
}

#[test]
fn a_file_over_64_mb_is_refused_by_its_size_without_being_opened_and_exactly_64_mb_is_not() {
    let folder = Scratch::new("size");
    let path = |name: &str| folder.path().join(name);
    let sparse = |name: &str, length: u64| {
        let file = std::fs::File::create(path(name)).expect("created");
        file.set_len(length).expect("a sparse file");
    };
    let over = |name: &str| format!("The settings file {} is larger than 64 MB, so it was not read.", shown(&path(name).to_string_lossy()));
    let limit = MAX_TOTAL_BYTES as u64;

    sparse("one-over.zip", limit + 1);
    no_answer(&path("one-over.zip"), &over("one-over.zip"));
    sparse("huge.zip", 5_000_000_000);
    no_answer(&path("huge.zip"), &over("huge.zip"));

    // The size decides before the file is opened: a file nobody may read still gets the size sentence.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        sparse("locked-big.zip", limit + 1);
        std::fs::set_permissions(path("locked-big.zip"), std::fs::Permissions::from_mode(0o000)).expect("permissions");
        no_answer(&path("locked-big.zip"), &over("locked-big.zip"));
    }

    // At exactly the limit the size is no reason to refuse: the bytes are read, and as they are not a
    // zip the core is the one that refuses.
    sparse("exact.zip", limit);
    for run in asked(&path("exact.zip")) {
        assert_eq!((run.code, run.out.as_str()), (1, ""));
        assert!(run.err.contains("was refused:") && !run.err.contains("larger than 64 MB"), "{}", run.err);
    }
}

#[test]
fn a_zip_with_no_blocks_is_a_real_settings_file_that_closes_nothing() {
    let folder = Scratch::new("empty-set");
    let file = write_settings(folder.path(), "no-blocks.zip", &[]);
    for run in asked(&file) {
        assert_eq!((run.code, run.err.as_str()), (0, ""), "{}", run.err);
        assert!(run.out.contains(" is open ") || run.out.contains("\"closed\":false"), "{}", run.out);
    }
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&file, folder.path().join("link.zip")).expect("a link");
        let linked = status(&folder.path().join("link.zip"), &["--app", "com.a", "--zone", "UTC"], NOW, None);
        assert_eq!(linked.out, status(&file, &["--app", "com.a", "--zone", "UTC"], NOW, None).out.replace("no-blocks", "link"), "a link to a settings file is read like the file");
    }
}
