//! `leko status` lists the parked changes of the blocks that cover the app or site asked about
//! (AC-010): one line each, in the words `leko settle` uses and in the zone chosen, before "Nothing was
//! saved or changed."; when nothing applies, or there is no waiting file, the sentence is the one status
//! always gave; status writes nothing and makes no file.

mod delay_runs;
#[path = "../../core/tests/delay_sets/mod.rs"]
mod delay_sets;
mod run_capture;
mod scratch_folder;
#[path = "../../core/tests/zip_writer/mod.rs"]
mod zip_writer;

use delay_runs::{settle, snapshot, status_blocks, status_records, waiting_of, write_waiting, write_zip, NOW};
use run_capture::status;
use scratch_folder::Scratch;
use std::path::PathBuf;

const NY: [&str; 2] = ["--zone", "America/New_York"];
const TAIL: &str = " Nothing was saved or changed.\n";

/// A folder with the settings and the parked changes, and another with the same settings only.
fn folders() -> (Scratch, PathBuf, Scratch, PathBuf) {
    let (with, without) = (Scratch::new("status-waiting"), Scratch::new("status-no-waiting"));
    let file = write_zip(with.path(), "s.zip", &status_blocks(), &[]);
    write_waiting(&file, &status_records());
    let plain = write_zip(without.path(), "s.zip", &status_blocks(), &[]);
    (with, file, without, plain)
}

fn answer(file: &std::path::Path, target: [&str; 2], zone: [&str; 2]) -> String {
    let run = status(file, &[&target[..], &zone[..]].concat(), NOW, None);
    assert_eq!((run.code, run.err.as_str()), (0, ""), "{target:?}: {}", run.err);
    run.out
}

/// `sentence` with its closing words cut off, ready for lines to be added.
fn head(sentence: &str) -> &str {
    sentence.strip_suffix(TAIL).unwrap_or_else(|| panic!("a status sentence ends with the closing words: {sentence:?}"))
}

#[test]
fn status_lists_the_waiting_changes_of_the_blocks_that_cover_the_target() {
    let (with, file, without, plain) = folders();
    let (before, plain_before) = (snapshot(with.path()), snapshot(without.path()));
    let line_s = "The block \"s\" is not due yet: it may be switched off from 2026-11-04T08:00:00-05:00 (America/New_York), held by: the block's own lock (a wait of 48 hours).";
    let line_a = "The block \"a\" is not due yet: it may be switched off from 2026-11-03T07:00:00-05:00 (America/New_York), held by: the block's own lock (a wait of 24 hours).";

    // A site that two blocks list: their lines, in file order, one per line, before the closing words.
    let site = ["--site", "example.com"];
    let sentence = answer(&file, site, NY);
    let bare = answer(&plain, site, NY);
    assert_eq!(sentence, format!("{}\n{line_s}\n{line_a}\nNothing was saved or changed.\n", head(&bare)));
    assert!(bare.matches('\n').count() == 1 && !bare.contains("block \"s\" is not due"), "without a waiting file the answer is one line, as it always was");

    // The very words `leko settle` uses for the same changes.
    let listed = settle(&file, &[], NOW);
    for line in [line_s, line_a] {
        assert!(listed.out.contains(&format!("{line}\n")), "settle says it too: {}", listed.out);
    }

    // Another site, an app, a change that is due, a lock that needs a key, a block that is already off.
    assert_eq!(answer(&file, ["--site", "other.org"], NY), format!("{}\n{line_s}\nNothing was saved or changed.\n", head(&answer(&plain, ["--site", "other.org"], NY))));
    assert_eq!(answer(&file, ["--app", "com.a"], NY), format!("{}\n{line_a}\nNothing was saved or changed.\n", head(&answer(&plain, ["--app", "com.a"], NY))));
    let due = "The block \"c\" is due and would be switched off.";
    assert_eq!(answer(&file, ["--app", "com.c"], NY), format!("{}\n{due}\nNothing was saved or changed.\n", head(&answer(&plain, ["--app", "com.c"], NY))));
    let key = "The block \"k\" now has a lock that needs a partner's key, so it was not switched off; its waiting change is kept.";
    assert_eq!(answer(&file, ["--site", "keyed.net"], NY), format!("{}\n{key}\nNothing was saved or changed.\n", head(&answer(&plain, ["--site", "keyed.net"], NY))));
    let off = "The block \"o\" is already off, so its waiting change is cleared.";
    assert_eq!(answer(&file, ["--app", "com.o"], NY), format!("{}\n{off}\nNothing was saved or changed.\n", head(&answer(&plain, ["--app", "com.o"], NY))));

    // Nothing applies: the sentence is byte for byte the old one, whatever the waiting file holds.
    for target in [["--app", "com.zzz"], ["--site", "nowhere.net"], ["--app", "example.com"], ["--app", "com.gone"], ["--site", "gone"]] {
        assert_eq!(answer(&file, target, NY), answer(&plain, target, NY), "{target:?}");
    }
    assert_eq!(
        answer(&file, ["--app", "com.zzz"], NY),
        "App \"com.zzz\" is open for at least the next 8 days, judged at 2026-11-02T10:00:00-05:00 in America/New_York. Apps that are never blocked are not applied yet. Nothing was saved or changed.\n"
    );

    // The moment is on the wall clock of the zone chosen, with its offset.
    let tokyo = answer(&file, site, ["--zone", "Asia/Tokyo"]);
    assert!(tokyo.contains("may be switched off from 2026-11-04T22:00:00+09:00 (Asia/Tokyo), held by:") && tokyo.contains("2026-11-03T21:00:00+09:00 (Asia/Tokyo)"), "{tokyo}");
    let computer = status(&file, &["--site", "example.com"], NOW, Some("America/New_York"));
    assert_eq!((computer.code, computer.err.as_str(), computer.out.as_str()), (0, "", sentence.as_str()), "the computer's zone is used when none is given");

    // Status writes nothing and makes no file.
    assert_eq!((snapshot(with.path()), snapshot(without.path())), (before, plain_before));
    assert!(!waiting_of(&plain).exists());
}
