//! Sites in canonical form and in the judge of loosening, run on the cases in
//! `extension-vectors/site-cases.json` with the extended schema: a repeated site is kept once,
//! removing a site loosens, adding one tightens, and reordering is no change.

mod vector_files;

use chrono::DateTime;
use leko_core::config_diff::item_diff::{diff_item, DiffContext};
use leko_core::config_diff::loosening_table::LooseningTable;
use leko_core::config_time::zone_time::ZoneTime;
use leko_core::domain::canonical::canonicalize;
use leko_core::domain::extended_schema::extended_schema;
use leko_core::domain::json_same::same_json;
use serde_json::Value;
use vector_files::read_extension_vector;

const CANONICAL_CASES: usize = 6;
const DIFF_CASES: usize = 18;

fn sign(loosens: bool) -> char {
    if loosens {
        '+'
    } else {
        '-'
    }
}

fn units(wanted: &[Value]) -> String {
    let mut named: Vec<String> = wanted.iter().map(|unit| format!("{}{}", unit["unit"].as_str().expect("unit"), sign(unit["loosens"] == true))).collect();
    named.sort();
    named.join(", ")
}

fn diff_problem(item: &Value, doc: &Value, table: &LooseningTable, zone: &ZoneTime) -> Option<String> {
    let canonical = |which: &str| canonicalize(extended_schema(), "block", &item[which]).expect("canonicalize");
    let at = DateTime::parse_from_rfc3339(doc["defaultAt"].as_str().expect("defaultAt")).expect("a time").timestamp_millis();
    let context = DiffContext { requested_g: at, zone, table };
    let found = diff_item("block", &canonical("old"), &canonical("new"), &context);
    let mut got: Vec<String> = found.iter().map(|unit| format!("{}{}", unit.path, sign(unit.loosens))).collect();
    got.sort();
    let (got, want) = (got.join(", "), units(item["expect"].as_array().expect("expect")));
    (got != want).then(|| format!("{}: expected [{want}], got [{got}]", item["name"]))
}

#[test]
fn site_cases_match_the_listed_answers() {
    let doc = read_extension_vector("site-cases.json");
    let table = LooseningTable::new(extended_schema()).expect("table");
    let zone = ZoneTime::new(doc["zone"].as_str().expect("zone")).expect("zone");
    assert_eq!(table.of("block.covers.sites"), "set-leave", "the direction is read from the extended schema");

    let canonical_cases = doc["canonical"].as_array().expect("canonical");
    assert_eq!(canonical_cases.len(), CANONICAL_CASES);
    let wrong_canonical: Vec<String> = canonical_cases
        .iter()
        .filter(|item| {
            let once = canonicalize(extended_schema(), "block", &item["doc"]).expect("canonicalize");
            let again = canonicalize(extended_schema(), "block", &once).expect("canonicalize");
            !same_json(&once["covers"], &item["canonicalCovers"]) || !same_json(&once, &again)
        })
        .map(|item| item["name"].as_str().unwrap_or("?").to_string())
        .collect();
    assert_eq!(wrong_canonical, Vec::<String>::new());

    let diff_cases = doc["diff"].as_array().expect("diff");
    assert_eq!(diff_cases.len(), DIFF_CASES);
    let wrong: Vec<String> = diff_cases.iter().filter_map(|item| diff_problem(item, &doc, &table, &zone)).collect();
    assert_eq!(wrong, Vec::<String>::new());
}
