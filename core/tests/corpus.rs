//! The phone's corpus of settings zips, run against this core.

mod corpus_zips;
mod zip_writer;

use corpus_zips::{case_bytes, entries_bytes, load_always_allowed, load_corpus};
use leko_core::domain::stored_state::{judge_stored_state, StoredVerdict};
use leko_core::io::zip_check::{check_zip, ZipVerdict};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use zip_writer::Method;

const CORPUS_SHA256: &str = "44be9d6672dca46a74f80ce83d8b2ae55bc6346286c25c8403b34d4a22fe126e";
const CORPUS_CASES: usize = 172;

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// `None` when the core's verdict on `bytes` is the one `item` stores, otherwise how it differs.
fn format_problem(item: &Value, bytes: &[u8], allowed: &HashSet<String>) -> Option<String> {
    let got = check_zip(bytes, allowed);
    let expect = &item["expect"];
    if let Some(want) = expect.get("read") {
        let mut want: Vec<&str> = want.as_array()?.iter().filter_map(Value::as_str).collect();
        want.sort_unstable();
        return match got {
            ZipVerdict::Refused(refusal) => Some(format!("expected {want:?}, refused: {}", refusal.sentence)),
            ZipVerdict::Read(read) if read.listing == want => None,
            ZipVerdict::Read(read) => Some(format!("expected {want:?}, read {:?}", read.listing)),
        };
    }
    let (entry, reason) = (expect["refused"]["entry"].as_str(), expect["refused"]["reason"].as_str()?);
    match got {
        ZipVerdict::Read(_) => Some(format!("expected a refusal naming {entry:?}, but it was read")),
        ZipVerdict::Refused(refusal) if refusal.entry.as_deref() == entry && refusal.sentence.contains(reason) => None,
        ZipVerdict::Refused(refusal) => Some(format!("expected [{entry:?}] with \"{reason}\", got [{:?}] \"{}\"", refusal.entry, refusal.sentence)),
    }
}

fn is_whole_file(item: &Value) -> bool {
    item["entries"].as_array().is_some_and(|entries| entries.iter().any(|entry| entry.get("raw").is_some()))
}

fn read_set(bytes: &[u8], allowed: &HashSet<String>, name: &str) -> leko_core::models::specs::ReadSet {
    match check_zip(bytes, allowed) {
        ZipVerdict::Read(read) => read.set,
        ZipVerdict::Refused(refusal) => panic!("{name}: both zips must read, but one was refused: {}", refusal.sentence),
    }
}

/// `None` when the stored-state verdict is the one `item` stores.
fn stored_problem(item: &Value, allowed: &HashSet<String>) -> Option<String> {
    let name = item["name"].as_str()?;
    let incoming = read_set(&case_bytes(item, Method::Deflate), allowed, name);
    let last_export = item["state"]["lastExport"].as_array()?;
    let stored = read_set(&entries_bytes(last_export, Method::Deflate), allowed, name);
    let got = judge_stored_state(&incoming, Some(&stored), item["state"]["protection"] == "on");
    let want = &item["expect"]["stored"];
    let ok = match &got {
        StoredVerdict::Accepted => want.get("accepted").is_some(),
        StoredVerdict::Refused(sentence) => want["refused"].as_str().is_some_and(|reason| sentence.contains(reason)),
    };
    (!ok).then(|| format!("{name}: expected {want}, got {got:?}"))
}

#[test]
fn corpus_file_is_the_pinned_copy() {
    let corpus = load_corpus();
    assert_eq!(hex(&Sha256::digest(&corpus.bytes)), CORPUS_SHA256, "vectors/corpus.json is not the phone's copy");
    assert_eq!(corpus.cases.len(), CORPUS_CASES, "the corpus must hold {CORPUS_CASES} cases");
}

#[test]
fn corpus_matches_all_172_cases() {
    let corpus = load_corpus();
    let allowed = load_always_allowed();
    assert_eq!(corpus.cases.len(), CORPUS_CASES);
    let mut failures: Vec<String> = Vec::new();
    for item in &corpus.cases {
        let name = item["name"].as_str().unwrap_or("?");
        // Written as the phone writes a zip.
        failures.extend(format_problem(item, &case_bytes(item, Method::Deflate), &allowed).map(|problem| format!("{name}: {problem}")));
        // The same entries, stored.
        if item.get("entries").is_some() && !is_whole_file(item) {
            failures.extend(format_problem(item, &case_bytes(item, Method::Stored), &allowed).map(|problem| format!("{name} (stored): {problem}")));
        }
        // Judged against the phone's stored state.
        if item.get("state").is_some() {
            failures.extend(stored_problem(item, &allowed));
        }
    }
    assert_eq!(failures, Vec::<String>::new());
    let stateful = corpus.cases.iter().filter(|item| item.get("state").is_some()).count();
    assert!(stateful >= 14, "the protection and key cases are in the corpus");
    assert_eq!(stateful, corpus.cases.iter().filter(|item| item["expect"].get("stored").is_some()).count());
}

#[test]
fn every_never_blocked_package_is_refused_in_a_block() {
    let allowed = load_always_allowed();
    assert!(allowed.len() >= 10, "the list has its packages");
    let block = |name: &str| {
        format!(r#"{{"schema":2,"id":"a","name":"A","covers":{{"apps":["{name}"]}},"rules":[{{"id":"m","kind":"minutes","amount":30,"per":"day","scope":"block"}}]}}"#)
    };
    let check = |name: &str| {
        let entries: Vec<Value> = vec![serde_json::json!({"name": "GUIDE.md", "text": "guide"}), serde_json::json!({"name": "blocks/a.json", "text": block(name)})];
        check_zip(&entries_bytes(&entries, Method::Deflate), &allowed)
    };
    for name in &allowed {
        let ZipVerdict::Refused(refusal) = check(name) else { panic!("{name} was read") };
        assert_eq!(refusal.entry.as_deref(), Some("blocks/a.json"));
        assert!(refusal.sentence.contains(&format!("cannot name {name}, which Enforcer never blocks")), "{name}");
        assert!(matches!(check(&format!("{name}.extra")), ZipVerdict::Read(_)), "{name}.extra is another app");
    }
}
