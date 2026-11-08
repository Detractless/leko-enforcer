//! Where a settings file uses something the phone 0.3 does not read: sites in a block's coverage, a
//! session rule, a moment seal. The phone refuses the whole zip for any of them, so an export can
//! use this list to say which files and fields to change. The field is named as the phone's own
//! refusal would name it.

use serde_json::Value;

/// One use of an addition: the file it is in, the field the phone would name, and what it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtensionUse {
    pub file: String,
    pub field: String,
    pub what: &'static str,
}

fn text_at<'a>(owner: &'a Value, key: &str, inner: &str) -> Option<&'a str> {
    owner.get(key)?.get(inner)?.as_str()
}

/// Every use of an addition in `docs`, a list of (zip path, parsed file), in file order.
pub fn extension_uses(docs: &[(String, Value)]) -> Vec<ExtensionUse> {
    let mut found = Vec::new();
    for (file, doc) in docs {
        let mut add = |field: String, what: &'static str| found.push(ExtensionUse { file: file.clone(), field, what });
        if doc.get("covers").is_some_and(|covers| covers.get("sites").is_some()) {
            add("covers.sites".to_string(), "sites");
        }
        if doc.get("device").is_some() {
            add("device".to_string(), "device block");
        }
        for (index, rule) in doc.get("rules").and_then(Value::as_array).into_iter().flatten().enumerate() {
            if rule.get("kind").and_then(Value::as_str) == Some("session") {
                add(format!("rules[{index}].kind"), "session rule");
            }
        }
        if text_at(doc, "lock", "level") == Some("sealedAt") {
            add("lock.level".to_string(), "moment seal");
        }
    }
    found
}
