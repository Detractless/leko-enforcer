//! The shared file of table-driven cases of the judge of loosening. The phone's own tests run the
//! same file, so a case here is a case of the judge on the phone.

mod vector_files;

use chrono::DateTime;
use leko_core::config::read_key_lock::read_lock;
use leko_core::config::strict_fields::StrictFields;
use leko_core::config_diff::item_diff::{diff_item, DiffContext};
use leko_core::config_diff::loosening_table::LooseningTable;
use leko_core::config_time::due_time::due;
use leko_core::config_time::zone_time::ZoneTime;
use leko_core::domain::canonical::canonicalize;
use leko_core::domain::schema_fields::bundled_schema;
use leko_core::models::specs::Lock;
use serde_json::{Map, Value};
use vector_files::read_vector;

fn instant(text: &str) -> i64 {
    DateTime::parse_from_rfc3339(text).unwrap_or_else(|_| panic!("{text} is not a time")).timestamp_millis()
}

fn kind_of(name: &str) -> &'static str {
    match name {
        "basics" => "general",
        "block" => "block",
        "practice" => "practice",
        "routine" => "routine",
        "deck" => "deck",
        other => panic!("no item kind {other}"),
    }
}

fn render(mut units: Vec<String>) -> String {
    units.sort();
    units.join(", ")
}

fn sign(loosens: bool) -> char {
    if loosens {
        '+'
    } else {
        '-'
    }
}

fn diff_problem(item: &Value, doc: &Value, table: &LooseningTable, zone: &ZoneTime) -> Option<String> {
    let kind = kind_of(item["kind"].as_str().expect("kind"));
    let schema = bundled_schema();
    let canonical = |which: &str| canonicalize(schema, kind, &item[which]).expect("canonicalize");
    let at = item["at"].as_str().or(doc["defaultAt"].as_str()).expect("a time");
    let context = DiffContext { requested_g: instant(at), zone, table };
    let got: Vec<String> = diff_item(kind, &canonical("old"), &canonical("new"), &context).iter().map(|unit| format!("{}{}", unit.path, sign(unit.loosens))).collect();
    let want: Vec<String> = item["expect"].as_array().expect("expect").iter().map(|unit| format!("{}{}", unit["unit"].as_str().expect("unit"), sign(unit["loosens"] == true))).collect();
    let (got, want) = (render(got), render(want));
    (got != want).then(|| format!("{}: expected [{want}], got [{got}]", item["name"]))
}

fn lock_of(lock: &Value) -> Lock {
    let mut file = Map::new();
    file.insert("lock".to_string(), lock.clone());
    read_lock(&mut StrictFields::new(&file, "")).unwrap_or_else(|problem| panic!("the lock {lock} is refused: {}", problem.sentence()))
}

fn due_problem(item: &Value, zone: &ZoneTime) -> Option<String> {
    let locks: Vec<Lock> = item["locks"].as_array().expect("locks").iter().map(lock_of).collect();
    let got = due(instant(item["at"].as_str().expect("at")), &locks, zone);
    let want = item["expect"].as_str().map(instant);
    (got != want).then(|| format!("{}: expected {want:?}, got {got:?}", item["name"]))
}

fn load() -> (Value, LooseningTable, ZoneTime) {
    let doc = read_vector("loosening-cases.json");
    let table = LooseningTable::new(bundled_schema()).expect("table");
    let zone = ZoneTime::new(doc["zone"].as_str().expect("zone")).expect("zone");
    (doc, table, zone)
}

#[test]
fn diff_matches_all_183_cases() {
    let (doc, table, zone) = load();
    let cases = doc["diff"].as_array().expect("diff");
    assert_eq!(cases.len(), 183);
    let wrong: Vec<String> = cases.iter().filter_map(|item| diff_problem(item, &doc, &table, &zone)).collect();
    assert_eq!(wrong, Vec::<String>::new());
}

#[test]
fn due_matches_all_28_cases() {
    let (doc, _, zone) = load();
    let cases = doc["due"].as_array().expect("due");
    assert_eq!(cases.len(), 28);
    let wrong: Vec<String> = cases.iter().filter_map(|item| due_problem(item, &zone)).collect();
    assert_eq!(wrong, Vec::<String>::new());
}

#[test]
fn the_judge_reads_its_directions_from_the_schema() {
    let (_, table, _) = load();
    assert_eq!(table.of("block.pass.perDay"), "up");
    assert_eq!(table.of("block.name"), "never");
    assert_eq!(table.of("general.emergency.perDay"), "emergency");
    assert_eq!(table.of("nothing.here"), "any");
    assert!(ZoneTime::new("Not/AZone").is_err(), "an unknown zone is an error");
}
