//! Switching a block off inside a settings zip: an unknown block, a block that is already off and a file
//! that is not a settings zip are refused in plain words, and the one that works changes only that block.

mod delay_sets;
mod zip_writer;

use delay_sets::{locked_block, off_block, wait, zip_with};
use leko_core::delayed_changes::block_off_zip::switch_block_off;
use leko_core::io::zip_reader::open_zip;
use serde_json::Value;

fn block_in(bytes: &[u8], id: &str) -> Value {
    let files = open_zip(bytes).expect("a zip").files;
    let (_, text) = files.iter().find(|(name, _)| *name == format!("blocks/{id}.json")).expect("the block's file");
    serde_json::from_slice(text).expect("JSON")
}

#[test]
fn only_a_block_that_is_on_can_be_switched_off_and_only_it_changes() {
    let bytes = zip_with(&[locked_block("a", wait(24)), locked_block("b", Value::Null), off_block("off")], &[]);

    assert_eq!(switch_block_off(&bytes, "nope"), Err("There is no block with the id \"nope\" in this file.".to_string()));
    assert_eq!(switch_block_off(&bytes, "off"), Err("The block \"off\" is already switched off.".to_string()));
    let refused = switch_block_off(b"not a zip at all", "a").expect_err("not a zip");
    assert!(refused.starts_with("The settings file was refused: "), "{refused}");

    let landed = switch_block_off(&bytes, "a").expect("a block that is on");
    assert_eq!(block_in(&landed, "a")["on"], Value::Bool(false));
    let mut expected = block_in(&bytes, "a");
    expected["on"] = Value::Bool(false);
    assert_eq!(block_in(&landed, "a"), expected, "only the `on` field of the block changed");
    for id in ["b", "off"] {
        assert_eq!(block_in(&landed, id), block_in(&bytes, id), "the block {id} was not touched");
    }
    let names = |zip: &[u8]| open_zip(zip).expect("a zip").files.into_iter().map(|(name, _)| name).collect::<Vec<_>>();
    assert_eq!(names(&landed), names(&bytes), "no file was added or lost");
    assert_eq!(switch_block_off(&landed, "a"), Err("The block \"a\" is already switched off.".to_string()), "switching off twice is refused");
}
