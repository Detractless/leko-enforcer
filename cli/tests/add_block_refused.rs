//! The core checks the draft and the new file before anything is shown or saved (AC-007): a package
//! or site its readers refuse, or a file already holding the most files a zip may hold, exits 1 with
//! the core's own sentence, shows no draft and saves nothing. The sentences are worked out here from
//! the core's reader on a zip built by the test, never taken from the command's output.

mod add_block_runs;
mod run_capture;
mod scratch_folder;
mod settings_builder;

use add_block_runs::{add, entries_of, saved_block, APP, NOW};
use leko_core::blocks::block_request::{BlockRequest, DoorChoice, RuleChoice};
use leko_core::blocks::draft_block::draft_block;
use leko_core::io::zip_check::{check_zip, ZipVerdict};
use leko_core::open_zip;
use run_capture::moment;
use scratch_folder::Scratch;
use serde_json::{json, Value};
use settings_builder::{always, block, settings_bytes, write_settings};
use std::collections::HashSet;
use std::path::Path;

const REFUSED: &str = "The settings file with the new block was refused: ";

fn old_block(id: &str) -> Value {
    block(id, json!({"apps": ["com.old.app"]}), json!([always("r", &[])]))
}

/// The request the flags `--name X --app/--site VALUE --always` stand for.
fn request(app: Option<&str>, site: Option<&str>) -> BlockRequest {
    BlockRequest {
        name: "X".into(),
        apps: app.map(str::to_string).into_iter().collect(),
        sites: site.map(str::to_string).into_iter().collect(),
        device: None,
        rule: RuleChoice::Always,
        days: vec![],
        door: DoorChoice::Default,
        message: None,
    }
}

/// The core's sentence for a settings file holding the old block and the block drafted for `request`.
fn core_sentence(request: &BlockRequest) -> String {
    let drafted = draft_block(request, moment(NOW)).expect("the draft is made; the readers refuse the file");
    match check_zip(&settings_bytes(&[old_block("old"), drafted.block]), &HashSet::new()) {
        ZipVerdict::Refused(refusal) => refusal.sentence,
        ZipVerdict::Read(_) => panic!("the core reads {request:?}"),
    }
}

/// Runs the flags with and without `--write`; each must exit 1 with `sentence` and leave the folder alone.
fn assert_refused_and_untouched(file: &Path, folder: &Scratch, flags: &[&str], sentence: &str, what: &str) {
    let bytes = std::fs::read(file).unwrap();
    for write in [false, true] {
        let flags = [flags, if write { &["--write"] } else { &[] }].concat();
        let run = add(file, &flags);
        assert_eq!((run.code, run.out.as_str()), (1, ""), "{what}, write {write}: no draft is shown: {}", run.err);
        assert!(run.err.starts_with(REFUSED) && run.err.contains(sentence), "{what}: the core's sentence is shown:\n{}\nexpected: {sentence}", run.err);
        assert!(run.err.ends_with(". Nothing was saved.\n") && run.err.lines().count() == 1, "{what}: {}", run.err);
        assert_eq!(std::fs::read(file).unwrap(), bytes, "{what}: the file is as it was");
        assert_eq!(folder.names(), ["settings.zip"], "{what}: no other file");
    }
}

#[test]
fn a_block_the_core_refuses_is_neither_shown_as_good_nor_saved() {
    let longest_ok = format!("{}b", "a.".repeat(127));
    let too_long = format!("{}bc", "a.".repeat(127));
    assert_eq!((longest_ok.len(), too_long.len()), (255, 256));
    let bad_apps = ["", &too_long, "has space", "com.example.", "two  words", "com..example"];
    let bad_sites = ["not_a_host", "http://example.org", "example", "Example.COM", "example.com/path", "*.example.com", "example.com:8080", "1.2.3.4", "exa mple.com"];
    for (label, app, site) in bad_apps.iter().map(|app| ("app", Some(*app), None)).chain(bad_sites.iter().map(|site| ("site", None, Some(*site)))) {
        let value = app.or(site).unwrap();
        let what = format!("{label} {value:?}");
        let core = core_sentence(&request(app, site));
        let folder = Scratch::new("refused");
        let file = write_settings(folder.path(), "settings.zip", &[old_block("old")]);
        let flags = ["--name", "X", if app.is_some() { "--app" } else { "--site" }, value, "--always"];
        assert_refused_and_untouched(&file, &folder, &flags, &core, &what);
    }

    // The longest package the reader takes is drafted and saved.
    let folder = Scratch::new("longest");
    let file = write_settings(folder.path(), "settings.zip", &[old_block("old")]);
    let run = add(&file, &["--name", "X", "--app", &longest_ok, "--always", "--write"]);
    assert_eq!((run.code, run.err.as_str()), (0, ""), "{}", run.out);
    assert_eq!(saved_block(&file, "x").unwrap()["covers"]["apps"], json!([longest_ok]));
}

#[test]
fn a_file_already_holding_the_most_files_refuses_one_more_and_one_below_takes_it() {
    // 500 files: GUIDE.md, general.json and 498 blocks. One more is 501, which no reader takes.
    let blocks_of = |count: usize| (0..count).map(|number| old_block(&format!("b{number:04}"))).collect::<Vec<_>>();
    let full = settings_bytes(&blocks_of(498));
    assert_eq!(open_zip(&full).expect("500 files are read").files.len(), 500);
    let over = open_zip(&settings_bytes(&blocks_of(499))).expect_err("501 files are refused");
    let folder = Scratch::new("full");
    let file = folder.path().join("settings.zip");
    std::fs::write(&file, &full).unwrap();
    assert_refused_and_untouched(&file, &folder, &["--name", "X", "--app", APP, "--always"], &over, "a full file");

    // 499 files take the 500th.
    let folder = Scratch::new("almost");
    let file = write_settings(folder.path(), "settings.zip", &blocks_of(497));
    let run = add(&file, &["--name", "X", "--app", APP, "--always", "--write"]);
    assert_eq!((run.code, run.err.as_str()), (0, ""), "{}", run.out);
    assert_eq!(entries_of(&file).len(), 500);
    assert!(saved_block(&file, "x").is_some());
}
