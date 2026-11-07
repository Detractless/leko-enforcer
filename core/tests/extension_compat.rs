//! The additions to format 2 (sites, session rules, moment seals) must change nothing for a file
//! that uses none of them: the phone's corpus, canonical cases, change cases and due-time cases
//! give the same answers with the additions switched on and switched off. A file that does use one
//! is refused, by name, by the reader that has them switched off.

mod corpus_zips;
mod vector_files;
mod zip_writer;

use chrono::DateTime;
use corpus_zips::{case_bytes, load_always_allowed, load_corpus};
use leko_core::config::read_key_lock::read_lock;
use leko_core::config::strict_fields::StrictFields;
use leko_core::config_diff::item_diff::{diff_item, DiffContext};
use leko_core::config_diff::loosening_table::LooseningTable;
use leko_core::config_time::due_time::due;
use leko_core::config_time::zone_time::ZoneTime;
use leko_core::domain::canonical::canonicalize;
use leko_core::domain::extended_schema::extended_schema;
use leko_core::domain::extension_fields::{extension_uses, ExtensionUse};
use leko_core::domain::json_same::same_json;
use leko_core::domain::schema_fields::bundled_schema;
use leko_core::io::zip_check::{check_zip_in_mode, ZipVerdict};
use leko_core::models::specs::{Lock, ReadMode};
use serde_json::{json, Map, Value};
use std::collections::HashSet;
use vector_files::read_vector;
use zip_writer::{zip_of, Method};

const CORPUS_CASES: usize = 172;
const CANONICAL_CASES: usize = 27;
const DIFF_CASES: usize = 183;
const DUE_CASES: usize = 28;

fn is_whole_file(item: &Value) -> bool {
    item["entries"].as_array().is_some_and(|entries| entries.iter().any(|entry| entry.get("raw").is_some()))
}

/// `None` when both modes give the same verdict on `bytes`, otherwise the two verdicts.
fn mode_difference(bytes: &[u8], allowed: &HashSet<String>) -> Option<String> {
    let phone = format!("{:?}", check_zip_in_mode(bytes, allowed, ReadMode::Phone));
    let extended = format!("{:?}", check_zip_in_mode(bytes, allowed, ReadMode::Extended));
    (phone != extended).then(|| format!("phone mode: {phone}\n  extended mode: {extended}"))
}

fn reader_differences() -> Vec<String> {
    let corpus = load_corpus();
    let allowed = load_always_allowed();
    assert_eq!(corpus.cases.len(), CORPUS_CASES);
    let mut differences: Vec<String> = Vec::new();
    for item in &corpus.cases {
        let name = item["name"].as_str().unwrap_or("?");
        differences.extend(mode_difference(&case_bytes(item, Method::Deflate), &allowed).map(|found| format!("{name}: {found}")));
        if item.get("entries").is_some() && !is_whole_file(item) {
            differences.extend(mode_difference(&case_bytes(item, Method::Stored), &allowed).map(|found| format!("{name} (stored): {found}")));
        }
    }
    differences
}

/// Canonical forms under the phone's schema and the extended schema must be the same, and the shared
/// file's own answer.
fn canonical_differences() -> Vec<String> {
    let cases = read_vector("canonical-cases.json")["cases"].as_array().expect("cases").clone();
    assert_eq!(cases.len(), CANONICAL_CASES);
    let mut differences = Vec::new();
    for item in &cases {
        let kind = item["kind"].as_str().expect("kind");
        let pinned = canonicalize(bundled_schema(), kind, &item["doc"]).expect("canonicalize");
        let extended = canonicalize(extended_schema(), kind, &item["doc"]).expect("canonicalize");
        if !same_json(&pinned, &extended) || !same_json(&extended, &item["canonical"]) {
            differences.push(format!("{}: pinned {pinned}, extended {extended}", item["name"]));
        }
    }
    differences
}

fn instant(text: &str) -> i64 {
    DateTime::parse_from_rfc3339(text).unwrap_or_else(|_| panic!("{text} is not a time")).timestamp_millis()
}

/// The judge's answers for one change case, with the schema (and so the table) given.
fn judged(item: &Value, at: &str, schema: &Value, zone: &ZoneTime) -> String {
    let kind = match item["kind"].as_str().expect("kind") {
        "basics" => "general",
        other => other,
    };
    let table = LooseningTable::new(schema).expect("table");
    let canonical = |which: &str| canonicalize(schema, kind, &item[which]).expect("canonicalize");
    let context = DiffContext { requested_g: instant(item["at"].as_str().unwrap_or(at)), zone, table: &table };
    format!("{:?}", diff_item(kind, &canonical("old"), &canonical("new"), &context))
}

fn diff_differences() -> Vec<String> {
    let doc = read_vector("loosening-cases.json");
    let zone = ZoneTime::new(doc["zone"].as_str().expect("zone")).expect("zone");
    let cases = doc["diff"].as_array().expect("diff");
    assert_eq!(cases.len(), DIFF_CASES);
    let at = doc["defaultAt"].as_str().expect("defaultAt");
    let mut differences = Vec::new();
    for item in cases {
        let (pinned, extended) = (judged(item, at, bundled_schema(), &zone), judged(item, at, extended_schema(), &zone));
        if pinned != extended {
            differences.push(format!("{}: pinned {pinned}, extended {extended}", item["name"]));
        }
    }
    differences
}

fn lock_in(mode: ReadMode, lock: &Value) -> Lock {
    let mut file = Map::new();
    file.insert("lock".to_string(), lock.clone());
    read_lock(&mut StrictFields::new(&file, "").with_mode(mode)).unwrap_or_else(|problem| panic!("the lock {lock} is refused: {}", problem.sentence()))
}

fn due_differences() -> Vec<String> {
    let doc = read_vector("loosening-cases.json");
    let zone = ZoneTime::new(doc["zone"].as_str().expect("zone")).expect("zone");
    let cases = doc["due"].as_array().expect("due");
    assert_eq!(cases.len(), DUE_CASES);
    let mut differences = Vec::new();
    for item in cases {
        let locks = |mode: ReadMode| -> Vec<Lock> { item["locks"].as_array().expect("locks").iter().map(|lock| lock_in(mode, lock)).collect() };
        let at = instant(item["at"].as_str().expect("at"));
        let (phone, extended) = (due(at, &locks(ReadMode::Phone), &zone), due(at, &locks(ReadMode::Extended), &zone));
        let want = item["expect"].as_str().map(instant);
        if phone != extended || extended != want {
            differences.push(format!("{}: phone mode {phone:?}, extended mode {extended:?}, expected {want:?}", item["name"]));
        }
    }
    differences
}

#[test]
fn format_2_results_are_the_same_in_both_modes() {
    assert_eq!(reader_differences(), Vec::<String>::new(), "the 172 reader cases");
    assert_eq!(canonical_differences(), Vec::<String>::new(), "the 27 canonical cases");
    assert_eq!(diff_differences(), Vec::<String>::new(), "the 183 change cases");
    assert_eq!(due_differences(), Vec::<String>::new(), "the 28 due-time cases");
}

const RULES: &str = r#"[{"id":"m","kind":"minutes","amount":30,"per":"day","scope":"block"}]"#;

fn block_with(covers: &str, rules: &str, lock: &str) -> Vec<u8> {
    let old = format!(r#"{{"schema":2,"id":"a","name":"A","covers":{{"apps":["com.a"]}},"rules":{RULES}}}"#);
    let block = format!(r#"{{"schema":2,"id":"b","name":"B","covers":{covers},"rules":{rules},"lock":{lock}}}"#);
    zip_of(&[("GUIDE.md", "guide"), ("blocks/a.json", &old), ("blocks/b.json", &block)])
}

/// The sentence of a refusal, or a panic when the zip was read.
fn refused_in(mode: ReadMode, bytes: &[u8]) -> String {
    match check_zip_in_mode(bytes, &HashSet::new(), mode) {
        ZipVerdict::Refused(refusal) => refusal.sentence,
        ZipVerdict::Read(_) => panic!("expected a refusal in {mode:?} mode, but the zip was read"),
    }
}

fn docs_of(bytes: &[u8]) -> Vec<(String, Value)> {
    match check_zip_in_mode(bytes, &HashSet::new(), ReadMode::Extended) {
        ZipVerdict::Read(read) => read.set.docs,
        ZipVerdict::Refused(refusal) => panic!("expected the zip to be read in extended mode: {}", refusal.sentence),
    }
}

fn use_of(file: &str, field: &str, what: &'static str) -> ExtensionUse {
    ExtensionUse { file: file.to_string(), field: field.to_string(), what }
}

#[test]
fn phone_mode_refuses_each_new_field_by_name() {
    let apps = r#"{"apps":["com.b"]}"#;
    let lock = r#"{"level":"open"}"#;
    let end = "2026-11-05T18:30:00Z";

    // Sites beside apps: the sentence is the one for any field the file does not have.
    let sites = block_with(r#"{"apps":["com.b"],"sites":["reddit.com"]}"#, RULES, lock);
    let invented = block_with(r#"{"apps":["com.b"],"zzz":["reddit.com"]}"#, RULES, lock);
    let sentence = refused_in(ReadMode::Phone, &sites);
    assert!(sentence.starts_with("blocks/b.json: ") && sentence.contains("\"covers.sites\" is not a field of this file"), "{sentence}");
    assert_eq!(sentence.replace("sites", "zzz"), refused_in(ReadMode::Phone, &invented), "the same sentence as for any other unknown field");

    // Sites alone: the phone's own sentence for a covers with neither list, which names covers.
    let alone = refused_in(ReadMode::Phone, &block_with(r#"{"sites":["reddit.com"]}"#, RULES, lock));
    assert_eq!(alone, refused_in(ReadMode::Phone, &block_with("{}", RULES, lock)), "sites alone is a covers with neither list");
    assert!(alone.starts_with("blocks/b.json: ") && alone.contains("covers"), "{alone}");

    // A session rule: the sentence for a rule kind the file does not know, naming rules[1].kind.
    let session = format!(r#"[{},{{"id":"s","kind":"session","endsAt":"{end}"}}]"#, &RULES[1..RULES.len() - 1]);
    let unknown_kind = session.replace("session\",\"endsAt", "bogus\",\"endsAt");
    let sentence = refused_in(ReadMode::Phone, &block_with(apps, &session, lock));
    assert!(sentence.starts_with("blocks/b.json: ") && sentence.contains("rules[1].kind") && sentence.contains("must be always, everyOpen, outsideHours, minutes or opens"), "{sentence}");
    assert_eq!(sentence.replace("session", "bogus"), refused_in(ReadMode::Phone, &block_with(apps, &unknown_kind, lock)).replace("session", "bogus"), "the same sentence as for any other unknown kind");

    // A moment seal: the sentence for a lock level the file does not know, naming lock.level.
    let seal = format!(r#"{{"level":"sealedAt","at":"{end}"}}"#);
    let sentence = refused_in(ReadMode::Phone, &block_with(apps, RULES, &seal));
    assert!(sentence.starts_with("blocks/b.json: ") && sentence.contains("lock.level") && sentence.contains("must be open, wait, changeHours or sealedUntil"), "{sentence}");
    assert_eq!(sentence, refused_in(ReadMode::Phone, &block_with(apps, RULES, r#"{"level":"bogus"}"#)), "the same sentence as for any other unknown level");

    // With the additions switched on, the same three files are read.
    for bytes in [&sites, &block_with(r#"{"sites":["reddit.com"]}"#, RULES, lock), &block_with(apps, &session, lock), &block_with(apps, RULES, &seal)] {
        assert!(matches!(check_zip_in_mode(bytes, &HashSet::new(), ReadMode::Extended), ZipVerdict::Read(_)));
    }

    // The list of uses names each place, in file order, as the phone's refusal names it.
    let everything = block_with(r#"{"sites":["reddit.com"]}"#, &session, &seal);
    assert_eq!(
        extension_uses(&docs_of(&everything)),
        vec![use_of("blocks/b.json", "covers.sites", "sites"), use_of("blocks/b.json", "rules[1].kind", "session rule"), use_of("blocks/b.json", "lock.level", "moment seal")]
    );
    assert_eq!(extension_uses(&docs_of(&sites)), vec![use_of("blocks/b.json", "covers.sites", "sites")]);
    assert_eq!(extension_uses(&docs_of(&block_with(apps, &session, lock))), vec![use_of("blocks/b.json", "rules[1].kind", "session rule")]);
    assert_eq!(extension_uses(&docs_of(&block_with(apps, RULES, &seal))), vec![use_of("blocks/b.json", "lock.level", "moment seal")]);
    assert_eq!(extension_uses(&docs_of(&block_with(apps, RULES, lock))), Vec::<ExtensionUse>::new());
    assert_eq!(extension_uses(&[("blocks/x.json".to_string(), json!({"covers": {"apps": []}, "lock": null, "rules": [{"kind": 5}, null]}))]), Vec::<ExtensionUse>::new(), "odd shapes are no use");

    // No file of the phone's corpus that the phone reads is listed as using an addition.
    let allowed = load_always_allowed();
    let listed: Vec<String> = load_corpus()
        .cases
        .iter()
        .filter_map(|item| match check_zip_in_mode(&case_bytes(item, Method::Deflate), &allowed, ReadMode::Phone) {
            ZipVerdict::Read(read) => Some((item["name"].as_str().unwrap_or("?").to_string(), extension_uses(&read.set.docs))),
            ZipVerdict::Refused(_) => None,
        })
        .filter(|(_, uses)| !uses.is_empty())
        .map(|(name, uses)| format!("{name}: {uses:?}"))
        .collect();
    assert_eq!(listed, Vec::<String>::new());
}
