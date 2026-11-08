//! A device block: a block that covers no app and no site but asks the system to lock the screen, log
//! the user out or shut down while its rule is closed. The reader accepts it only in the extended
//! mode, the phone refuses it by the field's name, the drafter writes it, and the strongest closed
//! device block is the action to take.

mod zip_writer;

use leko_core::blocks::block_request::{BlockRequest, DoorChoice, RuleChoice};
use leko_core::blocks::draft_block::draft_block;
use leko_core::config_time::zone_time::ZoneTime;
use leko_core::decision::device_action::{closed_device_rules, device_action, next_device_change};
use leko_core::io::zip_check::{check_zip, check_zip_in_mode, ZipVerdict};
use leko_core::models::specs::{AppCover, BlockSpec, DeviceAction, ReadMode, ReadSet};
use serde_json::{json, Value};
use std::collections::HashSet;
use zip_writer::zip_of;

const NOW: i64 = 1_793_000_000_000;

fn settings(blocks: &[Value]) -> Vec<u8> {
    let texts: Vec<(String, String)> = blocks
        .iter()
        .map(|block| (format!("blocks/{}.json", block["id"].as_str().unwrap()), serde_json::to_string(block).unwrap()))
        .collect();
    let mut files: Vec<(&str, &str)> = vec![("GUIDE.md", "guide"), ("general.json", r#"{"schema": 2}"#)];
    files.extend(texts.iter().map(|(name, text)| (name.as_str(), text.as_str())));
    zip_of(&files)
}

fn read(blocks: &[Value]) -> Result<ReadSet, String> {
    match check_zip(&settings(blocks), &HashSet::new()) {
        ZipVerdict::Read(read) => Ok(read.set),
        ZipVerdict::Refused(refusal) => Err(refusal.sentence),
    }
}

fn device_block(id: &str, device: &str, rule: Value, extra: Option<(&str, Value)>) -> Value {
    let mut block = json!({"schema": 2, "id": id, "name": id, "on": true, "device": device, "rules": [rule], "door": {"kind": "wall"}});
    if let Some((name, value)) = extra {
        block[name] = value;
    }
    block
}

fn zone() -> ZoneTime {
    ZoneTime::new("UTC").unwrap()
}

#[test]
fn a_device_block_reads_with_no_covers() {
    let set = read(&[device_block("freeze", "lock", json!({"id": "always", "kind": "always"}), None)]).expect("the extended reader takes a device block");
    let block: &BlockSpec = set.blocks.get("freeze").expect("the block is there");
    assert_eq!(block.device, Some(DeviceAction::Lock));
    assert_eq!(block.apps, AppCover::Nothing);
    assert!(block.sites.is_empty());
}

#[test]
fn the_phone_refuses_the_device_field_by_name() {
    let bytes = settings(&[device_block("freeze", "lock", json!({"id": "always", "kind": "always"}), None)]);
    match check_zip_in_mode(&bytes, &HashSet::new(), ReadMode::Phone) {
        ZipVerdict::Refused(refusal) => assert!(refusal.sentence.contains("device"), "{}", refusal.sentence),
        ZipVerdict::Read(_) => panic!("the phone must not read a device block"),
    }
}

#[test]
fn a_device_block_cannot_cover_an_app_or_a_site() {
    let covered = device_block("freeze", "lock", json!({"id": "always", "kind": "always"}), Some(("covers", json!({"apps": ["com.example.app"]}))));
    let sentence = read(&[covered]).expect_err("a device block covers nothing");
    assert!(sentence.contains("covers"), "{sentence}");
}

#[test]
fn a_device_block_has_no_way_in() {
    let door = device_block("freeze", "lock", json!({"id": "always", "kind": "always"}), Some(("pass", json!({"length": "minutes", "minutes": 5}))));
    let sentence = read(&[door]).expect_err("a device block has no pass");
    assert!(sentence.contains("pass"), "{sentence}");
}

#[test]
fn a_device_word_that_names_no_action_is_refused() {
    let bad = device_block("freeze", "nap", json!({"id": "always", "kind": "always"}), None);
    let sentence = read(&[bad]).expect_err("nap is not an action");
    assert!(sentence.contains("lock, logoff or shutdown"), "{sentence}");
}

#[test]
fn the_drafter_writes_a_device_block_the_readers_accept() {
    let mut request = BlockRequest { name: "Deep Work".into(), apps: vec![], sites: vec![], device: Some(DeviceAction::Lock), rule: RuleChoice::ForMinutes(90), days: vec![], door: DoorChoice::Default, message: None };
    let draft = draft_block(&request, NOW).expect("a device request is drafted");
    assert_eq!(draft.block["device"], json!("lock"));
    assert!(draft.block.get("covers").is_none(), "no covers are written");
    assert_eq!(draft.block["door"], json!({"kind": "wall"}));
    assert!(draft.ends_at.is_some(), "a timed block ends when its time is up");

    let set = read(&[draft.block.clone()]).expect("the drafter's file reads back");
    assert_eq!(set.blocks.get("deep-work").unwrap().device, Some(DeviceAction::Lock));

    request.rule = RuleChoice::Always;
    assert!(draft_block(&request, NOW).is_ok());
    request.apps = vec!["com.example.app".into()];
    assert!(draft_block(&request, NOW).is_err(), "a device block takes no app");
    request.apps.clear();
    request.door = DoorChoice::Wait(30);
    assert!(draft_block(&request, NOW).is_err(), "a device block has no way in");
}

#[test]
fn the_strongest_closed_device_block_acts() {
    let lock = device_block("a-lock", "lock", json!({"id": "always", "kind": "always"}), None);
    let off = device_block("b-logoff", "logoff", json!({"id": "always", "kind": "always"}), Some(("on", json!(false))));
    let ended = device_block("c-shutdown", "shutdown", json!({"id": "session", "kind": "session", "endsAt": "2026-10-01T00:00:00Z"}), None);
    let set = read(&[lock, off, ended]).expect("three device blocks are read");
    assert_eq!(device_action(&set, NOW, &zone()), Some(DeviceAction::Lock), "the switched-off and the ended block do nothing");

    let logoff = device_block("b-logoff", "logoff", json!({"id": "always", "kind": "always"}), None);
    let shutdown = device_block("c-shutdown", "shutdown", json!({"id": "always", "kind": "always"}), None);
    let set = read(&[logoff, shutdown]).expect("two closed device blocks are read");
    assert_eq!(device_action(&set, NOW, &zone()), Some(DeviceAction::Shutdown), "shutdown is stronger than logoff");
}

#[test]
fn the_closed_rules_and_the_next_change_are_found() {
    let lock = device_block("b-lock", "lock", json!({"id": "always", "kind": "always"}), None);
    let logoff = device_block("a-logoff", "logoff", json!({"id": "done", "kind": "session", "endsAt": "2026-10-26T08:00:00Z"}), None);
    let set = read(&[lock, logoff]).expect("two closed device blocks are read");

    let closing = closed_device_rules(&set, NOW, &zone());
    assert_eq!(
        closing,
        vec![
            ("a-logoff".to_string(), "done".to_string(), "session".to_string(), DeviceAction::Logoff),
            ("b-lock".to_string(), "always".to_string(), "always".to_string(), DeviceAction::Lock),
        ],
        "blocks in id order, rules as the file writes them"
    );

    let until = next_device_change(&set, NOW, &zone()).expect("the session ends within the look-ahead");
    assert_eq!(until, 1_793_001_600_000, "the end of the logoff session");

    let empty = read(&[]).expect("a settings file with no blocks reads");
    assert!(closed_device_rules(&empty, NOW, &zone()).is_empty());
    assert_eq!(next_device_change(&empty, NOW, &zone()), None);
}
