//! Switching a block on inside a settings zip (AC-002): an unknown block, a block that is already on
//! and a file that is not a settings zip are refused in plain words, and the one that works changes only
//! the `on` field of that block, in zips made here and in the phone-exported zips of the corpus.

mod corpus_zips;
mod delay_sets;
mod zip_writer;

use corpus_zips::{case_bytes, load_corpus};
use delay_sets::{locked_block, off_block, routine_holding, wait, zip_with};
use leko_core::delayed_changes::block_off_zip::switch_block_off;
use leko_core::io::zip_check::{check_zip, ZipVerdict};
use leko_core::io::zip_reader::open_zip;
use leko_core::switch_on::block_on_zip::switch_block_on;
use serde_json::Value;
use std::collections::HashSet;
use zip_writer::Method;

fn files(bytes: &[u8]) -> Vec<(String, Vec<u8>)> {
    open_zip(bytes).expect("a zip").files
}

fn block_in(bytes: &[u8], id: &str) -> Value {
    let list = files(bytes);
    let (_, text) = list.iter().find(|(name, _)| *name == format!("blocks/{id}.json")).expect("the block's file");
    serde_json::from_slice(text).expect("JSON")
}

/// Every file but `blocks/ID.json`, in order.
fn others(bytes: &[u8], id: &str) -> Vec<(String, Vec<u8>)> {
    files(bytes).into_iter().filter(|(name, _)| *name != format!("blocks/{id}.json")).collect()
}

fn reads(bytes: &[u8]) -> leko_core::models::specs::ReadSet {
    match check_zip(bytes, &HashSet::new()) {
        ZipVerdict::Read(read) => read.set,
        ZipVerdict::Refused(refusal) => panic!("refused: {}", refusal.sentence),
    }
}

#[test]
fn only_the_on_field_of_that_block_changes_and_the_new_zip_is_read_again() {
    let mut keyed = locked_block("k", wait(24 * 365));
    keyed["on"] = Value::Bool(false);
    let bytes = zip_with(&[keyed, off_block("a"), off_block("c"), locked_block("b", Value::Null)], &[routine_holding("w", &["k"], wait(24))]);
    let original = bytes.clone();

    assert_eq!(switch_block_on(&bytes, "nope"), Err("There is no block with the id \"nope\" in this file.".to_string()));
    assert_eq!(switch_block_on(&bytes, "A"), Err("There is no block with the id \"A\" in this file.".to_string()));
    assert_eq!(switch_block_on(&bytes, "b"), Err("The block \"b\" is already switched on.".to_string()), "no `on` field is on");
    let refused = switch_block_on(b"not a zip at all", "a").expect_err("not a zip");
    assert!(refused.starts_with("The settings file was refused: "), "{refused}");

    // A block with a wait of a year, held by a routine, switches on: only its `on` changes.
    for id in ["a", "k"] {
        let landed = switch_block_on(&bytes, id).expect("a block that is off");
        let mut expected = block_in(&bytes, id);
        expected["on"] = Value::Bool(true);
        assert_eq!(block_in(&landed, id), expected, "{id}: only the `on` field of the block changed");
        let (_, written) = files(&landed).into_iter().find(|(name, _)| *name == format!("blocks/{id}.json")).expect("the block's file");
        assert_eq!(written, format!("{}\n", serde_json::to_string_pretty(&expected).expect("JSON")).into_bytes(), "{id}: the file is the block as indented text with one closing line break, as a switch-off writes it");
        assert_eq!(others(&landed, id), others(&bytes, id), "{id}: every other file keeps its name, bytes and place");
        let names = |zip: &[u8]| files(zip).into_iter().map(|(name, _)| name).collect::<Vec<_>>();
        assert_eq!(names(&landed), names(&bytes), "{id}: no file was added, lost or moved");
        assert!(reads(&landed).blocks.get(id).is_some_and(|block| block.on), "{id}: the core reads the new zip as on");
        assert!(reads(&landed).blocks.get("c").is_some_and(|block| !block.on), "{id}: a neighbour that was off is still off");
        assert_eq!(switch_block_on(&landed, id), Err(format!("The block \"{id}\" is already switched on.")), "{id}: twice is refused");
    }
    assert_eq!(bytes, original, "the input was not changed");

    // Off and on again: the block ends on, and nothing else differs from what a switch-off left.
    let off = switch_block_off(&bytes, "b").expect("on to off");
    let back = switch_block_on(&off, "b").expect("off to on");
    assert_eq!(block_in(&back, "b")["on"], Value::Bool(true));
    assert_eq!(others(&back, "b"), others(&bytes, "b"));
}

#[test]
fn the_phone_exported_zips_switch_on_and_are_read_again() {
    let (mut switched, mut zips) = (0, 0);
    for item in load_corpus().cases.iter().filter(|item| item["expect"].get("read").is_some()) {
        let (case, bytes) = (item["name"].as_str().unwrap_or("?"), case_bytes(item, Method::Deflate));
        let ZipVerdict::Read(read) = check_zip(&bytes, &HashSet::new()) else { continue };
        zips += 1;
        for (id, block) in &read.set.blocks.items {
            let off = if block.on { switch_block_off(&bytes, id).unwrap_or_else(|why| panic!("{case} / {id}: {why}")) } else { bytes.clone() };
            let landed = switch_block_on(&off, id).unwrap_or_else(|why| panic!("{case} / {id}: {why}"));
            assert!(reads(&landed).blocks.get(id).is_some_and(|block| block.on), "{case} / {id}");
            assert_eq!(others(&landed, id), others(&bytes, id), "{case} / {id}: nothing else changed");
            switched += 1;
        }
    }
    assert!(zips >= 10 && switched >= 10, "the corpus was used: {switched} blocks in {zips} zips");
}
