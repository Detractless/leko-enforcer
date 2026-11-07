//! With `--write` and a change that lands now, the block's file gets `"on": false` and nothing else
//! changes (AC-004): every other file of the zip keeps its name and bytes, on built zips and on every
//! zip of the phone's corpus; the saved zip passes the core's check, and no other file is made.

mod delay_runs;
#[path = "../../core/tests/delay_sets/mod.rs"]
mod delay_sets;
mod run_capture;
mod scratch_folder;
#[path = "../../core/tests/zip_writer/mod.rs"]
mod zip_writer;
#[path = "../../core/tests/corpus_zips/mod.rs"]
mod corpus_zips;

use corpus_zips::{case_bytes, load_corpus};
use delay_runs::{assert_core_reads, assert_switched_off, block_of, entries_except, entries_of, switch_off, write_zip, NOW, ZONE};
use delay_sets::{locked_block, routine_holding, wait};
use leko_core::config_time::zone_time::ZoneTime;
use leko_core::delayed_changes::switch_off_plan::{plan_switch_off, SwitchOffPlan};
use leko_core::io::zip_check::{check_zip, ZipVerdict};
use run_capture::moment;
use scratch_folder::Scratch;
use serde_json::{json, Value};
use std::collections::HashSet;
use zip_writer::Method;

#[test]
fn write_switches_the_block_off_and_leaves_every_other_file_as_it_was() {
    // A built zip: a plain block, one with the `on` field written, a locked one and a routine.
    let folder = Scratch::new("write");
    let mut explicit = locked_block("explicit", Value::Null);
    explicit["on"] = json!(true);
    let blocks = [locked_block("a", Value::Null), explicit, locked_block("b", wait(24))];
    let file = write_zip(folder.path(), "s.zip", &blocks, &[routine_holding("r", &["b"], wait(24))]);
    for id in ["a", "explicit"] {
        let (before, old) = (entries_of(&file), block_of(&file, id));
        let run = switch_off(&file, &["--id", id, "--write"], NOW);
        assert_eq!((run.code, run.err.as_str()), (0, ""), "{id}");
        assert!(run.out.starts_with(&format!("Saved: the block \"{id}\" is switched off in ")) && run.out.ends_with('\n'), "{}", run.out);
        let after = entries_of(&file);
        assert_eq!(after.iter().map(|(name, _)| name).collect::<Vec<_>>(), before.iter().map(|(name, _)| name).collect::<Vec<_>>(), "{id}: the same files");
        assert_switched_off(&old, &block_of(&file, id));
        let kept = |entries: &[(String, Vec<u8>)]| entries.iter().filter(|(name, _)| *name != format!("blocks/{id}.json")).cloned().collect::<Vec<_>>();
        assert_eq!(kept(&after), kept(&before), "{id}: every other file keeps its bytes");
        assert_core_reads(&file);
        assert_eq!(folder.names(), ["s.zip"], "{id}: no other file is made");
    }

    // The corpus: every block of every zip the core reads that would land now is switched off, and
    // nothing else in the zip changes.
    let zone = ZoneTime::new(ZONE).expect("a zone");
    let (mut switched, mut skipped) = (0, 0);
    for item in load_corpus().cases.iter().filter(|item| item["expect"].get("read").is_some()) {
        let (name, bytes) = (item["name"].as_str().unwrap_or("?"), case_bytes(item, Method::Deflate));
        let ZipVerdict::Read(read) = check_zip(&bytes, &HashSet::new()) else { skipped += 1; continue };
        let ids: Vec<String> = read.set.docs.iter().filter_map(|(path, _)| path.strip_prefix("blocks/")?.strip_suffix(".json").map(str::to_string)).collect();
        for id in ids.iter().filter(|id| plan_switch_off(&read.set, id, moment(NOW), &zone) == SwitchOffPlan::LandsNow) {
            let folder = Scratch::new("corpus");
            let file = folder.path().join("s.zip");
            std::fs::write(&file, &bytes).unwrap();
            let (before, old) = (entries_of(&file), block_of(&file, id));
            let run = switch_off(&file, &["--id", id, "--write"], NOW);
            assert_eq!((run.code, run.err.as_str()), (0, ""), "{name} / {id}: {}", run.out);
            assert_switched_off(&old, &block_of(&file, id));
            assert_eq!(entries_except(&file, &[id]), before.into_iter().filter(|(file_name, _)| *file_name != format!("blocks/{id}.json")).collect::<Vec<_>>(), "{name} / {id}");
            assert_core_reads(&file);
            assert_eq!(folder.names(), ["s.zip"], "{name} / {id}");
            switched += 1;
        }
    }
    assert!(switched >= 3, "the corpus was used: {switched} blocks switched off, {skipped} zips skipped");
}
