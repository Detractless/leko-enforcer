//! Walks the written schema field by field. A field's path is dotted from its file kind
//! ("block.pass.perDay"), a list element is written [] ("block.rules[].amount"), and a field takes
//! the part and the loosening note of the field that holds it when it has none of its own.
//! Ported from `lib/schema-fields.js`.

use serde_json::{Map, Value};
use std::sync::OnceLock;

/// The schema this core reads, the phone's written copy (pinned by hash in the tests).
const BUNDLED: &str = include_str!("../../../vectors/enforcer-config-v2-schema.json");

/// The bundled schema, parsed once.
pub fn bundled_schema() -> &'static Value {
    static SCHEMA: OnceLock<Value> = OnceLock::new();
    SCHEMA.get_or_init(|| serde_json::from_str(BUNDLED).unwrap_or(Value::Null))
}

/// One field of one file kind.
#[derive(Debug, Clone)]
pub struct Field {
    pub path: String,
    pub def: String,
    pub name: String,
    pub node: Value,
    pub required: bool,
    pub part: Option<String>,
    pub loosening: Option<String>,
}

/// The definition `name`, or a sentence saying the schema has none.
pub fn definition<'a>(root: &'a Value, name: &str) -> Result<&'a Value, String> {
    root.get("$defs").and_then(|defs| defs.get(name)).filter(|found| !found.is_null()).ok_or_else(|| format!("the schema has no definition \"{name}\""))
}

/// The name a `$ref` points at: the text after its last slash.
pub fn ref_name(node: &Value) -> Option<&str> {
    let reference = node.get("$ref")?.as_str()?;
    Some(reference.rfind('/').map_or(reference, |at| &reference[at + 1..]))
}

/// `node` with its `$ref` replaced by the referenced definition, the field's own keys winning.
pub fn resolve_node(root: &Value, node: &Value) -> Result<Value, String> {
    let Some(name) = ref_name(node) else { return Ok(node.clone()) };
    let mut merged: Map<String, Value> = definition(root, name)?.as_object().cloned().unwrap_or_default();
    for (key, value) in node.as_object().into_iter().flatten().filter(|(key, _)| key.as_str() != "$ref") {
        merged.insert(key.clone(), value.clone());
    }
    Ok(Value::Object(merged))
}

fn has(node: &Value, key: &str) -> bool {
    node.get(key).is_some_and(|value| !value.is_null())
}

fn inner_object(root: &Value, node: &Value) -> Result<Option<Value>, String> {
    if has(node, "properties") {
        return Ok(Some(node.clone()));
    }
    let Some(items) = node.get("items").filter(|items| !items.is_null()) else { return Ok(None) };
    let items = resolve_node(root, items)?;
    Ok(has(&items, "properties").then_some(items))
}

fn text_of(node: &Value, key: &str) -> Option<String> {
    node.get(key).and_then(Value::as_str).map(str::to_string)
}

fn walk(root: &Value, def: &str, owner: &Value, prefix: &str, part: &Option<String>, loosening: &Option<String>) -> Result<Vec<Field>, String> {
    let required: Vec<&str> = owner.get("required").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str).collect();
    let mut out = Vec::new();
    for (name, declared) in owner.get("properties").and_then(Value::as_object).into_iter().flatten() {
        let node = resolve_node(root, declared)?;
        let own_part = text_of(&node, "x-part").or_else(|| part.clone());
        let own_loosening = text_of(&node, "x-loosening").or_else(|| loosening.clone());
        let path = format!("{prefix}.{name}");
        let field = Field { path: path.clone(), def: def.to_string(), name: name.clone(), node: node.clone(), required: required.contains(&name.as_str()), part: own_part.clone(), loosening: own_loosening.clone() };
        out.push(field);
        if let Some(inner) = inner_object(root, &node)? {
            let items = node.get("items").filter(|items| !items.is_null());
            let inner_def = ref_name(declared).or_else(|| items.and_then(ref_name)).unwrap_or(def);
            let inner_prefix = if items.is_some() { format!("{path}[]") } else { path };
            out.extend(walk(root, inner_def, &inner, &inner_prefix, &own_part, &own_loosening)?);
        }
    }
    Ok(out)
}

/// The file kinds the schema describes, in its order: general, block, practice, routine, deck, book.
pub fn file_kinds(root: &Value) -> Result<Vec<String>, String> {
    let files = root.get("x-files").and_then(Value::as_object).ok_or_else(|| "the schema lists no files (x-files)".to_string())?;
    Ok(files.keys().cloned().collect())
}

/// Every field of every file kind, parents before their fields.
pub fn all_fields(root: &Value) -> Result<Vec<Field>, String> {
    let mut out = Vec::new();
    for kind in file_kinds(root)? {
        out.extend(walk(root, &kind, definition(root, &kind)?, &kind, &None, &None)?);
    }
    Ok(out)
}
