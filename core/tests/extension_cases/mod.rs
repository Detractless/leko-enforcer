//! Runs the case lists made for Leko Enforcer's additions (`extension-vectors/*.json`) with the
//! extended schema: the change cases through the judge of loosening, the due-time cases through
//! the landing time. A case can name its own moment (`at`); otherwise the file's `defaultAt` is used.
#![allow(dead_code)]

use chrono::DateTime;
use leko_core::config::read_key_lock::read_lock;
use leko_core::config::strict_fields::StrictFields;
use leko_core::config_diff::item_diff::{diff_item, DiffContext};
use leko_core::config_diff::loosening_table::LooseningTable;
use leko_core::config_time::due_time::due;
use leko_core::config_time::zone_time::ZoneTime;
use leko_core::domain::canonical::canonicalize;
use leko_core::domain::extended_schema::extended_schema;
use leko_core::models::specs::{Lock, ReadMode};
use serde_json::{Map, Value};

use crate::vector_files::read_extension_vector;

/// Zones far apart in offset and in clock-change rules, for answers that must not depend on the zone.
pub const ZONES: [&str; 6] = ["America/New_York", "Asia/Tokyo", "Pacific/Auckland", "UTC", "Pacific/Kiritimati", "America/Sao_Paulo"];

pub struct CaseFile {
    pub doc: Value,
    pub table: LooseningTable,
    pub zone: ZoneTime,
}

pub fn load(name: &str) -> CaseFile {
    let doc = read_extension_vector(name);
    let table = LooseningTable::new(extended_schema()).expect("table");
    let zone = ZoneTime::new(doc["zone"].as_str().expect("zone")).expect("zone");
    CaseFile { doc, table, zone }
}

pub fn instant(text: &str) -> i64 {
    DateTime::parse_from_rfc3339(text).unwrap_or_else(|_| panic!("{text} is not a time")).timestamp_millis()
}

fn sign(loosens: bool) -> char {
    if loosens {
        '+'
    } else {
        '-'
    }
}

fn listed(units: &[(String, bool)]) -> String {
    let mut named: Vec<String> = units.iter().map(|(unit, loosens)| format!("{unit}{}", sign(*loosens))).collect();
    named.sort();
    named.join(", ")
}

/// The change cases that do not give the listed answer, each as a sentence.
pub fn diff_problems(file: &CaseFile, expected_count: usize) -> Vec<String> {
    let cases = file.doc["diff"].as_array().expect("diff");
    assert_eq!(cases.len(), expected_count, "the number of change cases");
    let default_at = file.doc["defaultAt"].as_str().expect("defaultAt");
    let mut problems = Vec::new();
    for item in cases {
        let canonical = |which: &str| canonicalize(extended_schema(), "block", &item[which]).expect("canonicalize");
        let context = DiffContext { requested_g: instant(item["at"].as_str().unwrap_or(default_at)), zone: &file.zone, table: &file.table };
        let found = diff_item("block", &canonical("old"), &canonical("new"), &context);
        let got = listed(&found.iter().map(|unit| (unit.path.clone(), unit.loosens)).collect::<Vec<_>>());
        let want = listed(&item["expect"].as_array().expect("expect").iter().map(|unit| (unit["unit"].as_str().expect("unit").to_string(), unit["loosens"] == true)).collect::<Vec<_>>());
        if got != want {
            problems.push(format!("{}: expected [{want}], got [{got}]", item["name"]));
        }
    }
    problems
}

/// A lock written as file JSON, read with the additions switched on.
pub fn extended_lock(lock: &Value) -> Lock {
    let mut file = Map::new();
    file.insert("lock".to_string(), lock.clone());
    read_lock(&mut StrictFields::new(&file, "").with_mode(ReadMode::Extended)).unwrap_or_else(|problem| panic!("the lock {lock} is refused: {}", problem.sentence()))
}

/// The landing time of one due-time case in `zone`.
pub fn landing(item: &Value, zone: &ZoneTime) -> Option<i64> {
    let locks: Vec<Lock> = item["locks"].as_array().expect("locks").iter().map(extended_lock).collect();
    due(instant(item["at"].as_str().expect("at")), &locks, zone)
}
