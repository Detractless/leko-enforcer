//! The canonical form: every default is written, and the shared file of documents and canonical
//! forms (which the phone's own tests read as well) gives the form this core writes. Defaults come
//! from the `default` keyword of the schema.

mod vector_files;

use leko_core::domain::canonical::canonicalize;
use leko_core::domain::json_same::same_json;
use leko_core::domain::schema_fields::{all_fields, bundled_schema};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use vector_files::{hex, read_vector, vector_bytes};

const SCHEMA_SHA256: &str = "9ce69e119fd82454dc3d3dd53890fb74485588fcb38e6c83ad5e5c3757be9af2";
const CASES: usize = 27;

fn cases() -> Vec<Value> {
    read_vector("canonical-cases.json")["cases"].as_array().expect("cases").clone()
}

#[test]
fn canonical_matches_all_27_cases() {
    let schema = bundled_schema();
    let cases = cases();
    assert_eq!(cases.len(), CASES);
    let wrong: Vec<String> = cases
        .iter()
        .filter(|item| !same_json(&canonicalize(schema, item["kind"].as_str().expect("kind"), &item["doc"]).expect("canonicalize"), &item["canonical"]))
        .map(|item| item["name"].as_str().unwrap_or("?").to_string())
        .collect();
    assert_eq!(wrong, Vec::<String>::new());
}

#[test]
fn a_canonical_form_is_stable_and_the_input_is_not_changed() {
    let schema = bundled_schema();
    for item in cases() {
        let kind = item["kind"].as_str().expect("kind");
        let before = item["doc"].clone();
        let once = canonicalize(schema, kind, &item["doc"]).expect("canonicalize");
        assert_eq!(item["doc"], before, "{}: the input changed", item["name"]);
        assert!(same_json(&canonicalize(schema, kind, &once).expect("canonicalize"), &once), "{}: not stable", item["name"]);
    }
}

#[test]
fn something_that_is_not_an_object_comes_back_as_it_is() {
    let schema = bundled_schema();
    assert_eq!(canonicalize(schema, "block", &Value::Null), Ok(Value::Null));
    assert_eq!(canonicalize(schema, "block", &json!([1])), Ok(json!([1])));
}

#[test]
fn the_bundled_schema_is_the_pinned_copy_and_lists_its_fields() {
    assert_eq!(hex(&Sha256::digest(vector_bytes("enforcer-config-v2-schema.json"))), SCHEMA_SHA256, "the schema is not the phone's copy");
    let from_file = read_vector("enforcer-config-v2-schema.json");
    assert_eq!(bundled_schema(), &from_file, "the bundled schema is the file in vectors/");
    let fields = all_fields(bundled_schema()).expect("fields");
    assert!(fields.iter().any(|field| field.path == "block.pass.perDay"));
    assert!(fields.iter().any(|field| field.path.starts_with("block.rules[].")));
    assert!(canonicalize(bundled_schema(), "nonsense", &json!({})).is_err(), "an unknown kind is an error");
}
