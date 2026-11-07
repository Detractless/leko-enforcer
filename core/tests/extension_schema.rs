//! The extended schema differs from the phone's pinned format-2 schema only in the places the
//! additions need, and the pinned file itself is unchanged.

mod vector_files;

use leko_core::domain::extended_schema::extended_schema;
use leko_core::domain::schema_fields::bundled_schema;
use serde_json::Value;
use sha2::{Digest, Sha256};
use vector_files::{hex, vector_bytes};

const SCHEMA_SHA256: &str = "9ce69e119fd82454dc3d3dd53890fb74485588fcb38e6c83ad5e5c3757be9af2";

/// `covers.sites`, and a covers with sites alone (the pinned "exactly one of apps, allAppsExcept" is replaced).
const SITES: [&str; 6] = [
    "/$defs/block/properties/covers/description",
    "/$defs/covers/allOf",
    "/$defs/covers/description",
    "/$defs/covers/oneOf",
    "/$defs/covers/properties/sites",
    "/$defs/site",
];

/// Rule kind `session` with `endsAt`: the kind list, the new property, the new clause, and `endsAt`
/// ruled out for the other four kinds; the block's rule list says at most one session.
const SESSION: [&str; 9] = [
    "/$defs/block/properties/rules/description",
    "/$defs/rule/allOf/0/then/not/anyOf/5",
    "/$defs/rule/allOf/1/then/not/anyOf/4",
    "/$defs/rule/allOf/2/then/not/anyOf/2",
    "/$defs/rule/allOf/3/then/not/anyOf/2",
    "/$defs/rule/allOf/4",
    "/$defs/rule/properties/endsAt",
    "/$defs/rule/properties/kind/description",
    "/$defs/rule/properties/kind/enum/5",
];

/// Lock level `sealedAt` with `at`: the level list, the new property, the new clause.
const SEAL: [&str; 4] = ["/$defs/lock/allOf/3", "/$defs/lock/description", "/$defs/lock/properties/at", "/$defs/lock/properties/level/enum/4"];

/// Every place where `extended` is not `pinned`: a changed value, a key or list item only one has.
/// Lists are compared item by item, so a clause added at the end shows as its own place.
fn differences(path: &str, pinned: &Value, extended: &Value, found: &mut Vec<String>) {
    match (pinned, extended) {
        (Value::Object(was), Value::Object(now)) => {
            for key in was.keys().chain(now.keys().filter(|key| !was.contains_key(*key))) {
                match (was.get(key), now.get(key)) {
                    (Some(old), Some(new)) => differences(&format!("{path}/{key}"), old, new, found),
                    _ => found.push(format!("{path}/{key}")),
                }
            }
        }
        (Value::Array(was), Value::Array(now)) => {
            for index in 0..was.len().max(now.len()) {
                match (was.get(index), now.get(index)) {
                    (Some(old), Some(new)) => differences(&format!("{path}/{index}"), old, new, found),
                    _ => found.push(format!("{path}/{index}")),
                }
            }
        }
        (old, new) => {
            if old != new {
                found.push(path.to_string());
            }
        }
    }
}

#[test]
fn extended_schema_changes_only_the_listed_places() {
    assert_eq!(hex(&Sha256::digest(vector_bytes("enforcer-config-v2-schema.json"))), SCHEMA_SHA256, "the pinned schema is the phone's copy, unchanged");
    let mut found = Vec::new();
    differences("", bundled_schema(), extended_schema(), &mut found);
    found.sort();
    let mut listed: Vec<String> = SITES.iter().chain(SESSION.iter()).chain(SEAL.iter()).map(|place| place.to_string()).collect();
    listed.sort();
    assert_eq!(found, listed);

    // The new rule field is judged with the other rule fields: same part of the file, same direction.
    let property = |name: &str| extended_schema()["$defs"]["rule"]["properties"][name].clone();
    for key in ["x-part", "x-loosening"] {
        assert_eq!(property("endsAt")[key], property("days")[key], "endsAt carries {key} like the other rule fields");
        assert!(property("endsAt")[key].is_string(), "{key} is set on endsAt");
    }
}
