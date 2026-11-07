//! Builds the bytes of each case of the shared corpus. An entry whose name appears twice, or whose
//! name is not UTF-8, is written under a name of the same length and renamed in the finished
//! bytes, since a zip tool would not write it. A case can also carry the exact bytes of its zip
//! (`zipBase64`). Any key the builder does not know panics, so no case can be skipped without
//! anyone noticing. Ported from `test/corpus-zips.js`.
#![allow(dead_code)]

use super::zip_writer::{build_zip, Entry, Method};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use serde_json::Value;
use std::collections::HashSet;

const CASE_KEYS: [&str; 6] = ["name", "entries", "zipBase64", "generate", "state", "expect"];
const GENERATE_KEYS: [&str; 2] = ["files", "bytesEach"];
const STATE_KEYS: [&str; 2] = ["protection", "lastExport"];
const EXPECT_KEYS: [&str; 3] = ["read", "refused", "stored"];
const ENTRY_KEYS: [&str; 7] = ["name", "text", "dir", "raw", "whole", "duplicate", "nameHex"];

pub fn vector_path(name: &str) -> String {
    format!("{}/../vectors/{name}", env!("CARGO_MANIFEST_DIR"))
}

pub struct Corpus {
    pub bytes: Vec<u8>,
    pub cases: Vec<Value>,
}

fn assert_known(object: &Value, allowed: &[&str], place: &str) {
    for key in object.as_object().unwrap_or_else(|| panic!("{place} is not an object")).keys() {
        assert!(allowed.contains(&key.as_str()), "{place} has the key \"{key}\", which this harness does not handle");
    }
}

/// Panics when a case, its state, its verdict or one of its entries has a key this harness does not handle.
pub fn check_keys(item: &Value) {
    let name = item["name"].as_str().unwrap_or("?");
    assert_known(item, &CASE_KEYS, &format!("the case \"{name}\""));
    assert_known(&item["expect"], &EXPECT_KEYS, &format!("the verdict of \"{name}\""));
    if item.get("state").is_some() {
        assert_known(&item["state"], &STATE_KEYS, &format!("the state of \"{name}\""));
    }
    if item.get("generate").is_some() {
        assert_known(&item["generate"], &GENERATE_KEYS, &format!("the generator of \"{name}\""));
    }
    let own = item.get("entries").and_then(Value::as_array).into_iter().flatten();
    let last = item.pointer("/state/lastExport").and_then(Value::as_array).into_iter().flatten();
    for entry in own.chain(last) {
        assert_known(entry, &ENTRY_KEYS, &format!("an entry of \"{name}\""));
    }
}

/// The corpus file: its bytes and its parsed cases, every key checked.
pub fn load_corpus() -> Corpus {
    let bytes = std::fs::read(vector_path("corpus.json")).expect("vectors/corpus.json is missing");
    let doc: Value = serde_json::from_slice(&bytes).expect("corpus.json is not JSON");
    let cases = doc["cases"].as_array().expect("corpus.json has no cases list").clone();
    cases.iter().for_each(check_keys);
    Corpus { bytes, cases }
}

/// The packages Enforcer never blocks.
pub fn load_always_allowed() -> HashSet<String> {
    let doc: Value = serde_json::from_slice(&std::fs::read(vector_path("always-allowed.json")).expect("always-allowed.json is missing")).expect("always-allowed.json is not JSON");
    doc["packages"].as_array().expect("no packages list").iter().map(|name| name.as_str().expect("a package is text").to_string()).collect()
}

fn from_hex(text: &str) -> Vec<u8> {
    (0..text.len()).step_by(2).map(|at| u8::from_str_radix(&text[at..at + 2], 16).expect("nameHex is hex")).collect()
}

fn placeholder(length: usize, index: usize) -> Vec<u8> {
    assert!(length >= 4, "a renamed entry needs a name of 4 bytes or more");
    let mut name = format!("Q{index}").into_bytes();
    name.resize(length, b'Q');
    name
}

fn replace_all(bytes: &mut [u8], from: &[u8], to: &[u8]) {
    let mut at = 0;
    while at + from.len() <= bytes.len() {
        if &bytes[at..at + from.len()] == from {
            bytes[at..at + from.len()].copy_from_slice(to);
            at += from.len();
        } else {
            at += 1;
        }
    }
}

/// The bytes of the zip that `entries` describe, written as `method` says.
pub fn entries_bytes(entries: &[Value], method: Method) -> Vec<u8> {
    if let Some(raw) = entries.iter().find_map(|entry| entry.get("raw")) {
        return raw.as_str().expect("raw is text").as_bytes().to_vec();
    }
    let mut patches: Vec<(Vec<u8>, Vec<u8>)> = Vec::new();
    let mut written = Vec::new();
    for entry in entries {
        let target = match entry.get("nameHex") {
            Some(hex) => from_hex(hex.as_str().expect("nameHex is text")),
            None => entry["name"].as_str().expect("an entry has a name").as_bytes().to_vec(),
        };
        let renamed = entry.get("nameHex").is_some() || entry.get("duplicate") == Some(&Value::Bool(true));
        let name = if renamed { placeholder(target.len(), patches.len()) } else { target.clone() };
        if renamed {
            patches.push((name.clone(), target));
        }
        let data = if entry.get("dir") == Some(&Value::Bool(true)) { Vec::new() } else { entry.get("text").and_then(Value::as_str).unwrap_or("").as_bytes().to_vec() };
        written.push(Entry { name, data });
    }
    let mut bytes = build_zip(&written, method);
    for (from, to) in &patches {
        replace_all(&mut bytes, from, to);
    }
    bytes
}

/// Files named f0.bin, f1.bin, ... of `bytes_each` bytes each, every byte the character 0.
fn generated_bytes(files: usize, bytes_each: usize, method: Method) -> Vec<u8> {
    let data = vec![0x30u8; bytes_each];
    let entries: Vec<Entry> = (0..files).map(|index| Entry { name: format!("f{index}.bin").into_bytes(), data: data.clone() }).collect();
    build_zip(&entries, method)
}

/// The bytes of one corpus case.
pub fn case_bytes(item: &Value, method: Method) -> Vec<u8> {
    if let Some(encoded) = item.get("zipBase64") {
        return STANDARD.decode(encoded.as_str().expect("zipBase64 is text")).expect("zipBase64 is base64");
    }
    if let Some(generate) = item.get("generate") {
        let number = |key: &str| generate[key].as_u64().expect("a generator count") as usize;
        return generated_bytes(number("files"), number("bytesEach"), method);
    }
    entries_bytes(item["entries"].as_array().expect("a case has entries"), method)
}
