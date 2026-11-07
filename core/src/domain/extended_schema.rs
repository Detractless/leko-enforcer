//! The extended schema: the phone's format-2 schema plus what Leko Enforcer reads on top of it. It is
//! a second written file, `extension-vectors/leko-config-extended.schema.json`; the phone's own
//! copy in `vectors/` is never changed.

use serde_json::Value;
use std::sync::OnceLock;

const EXTENDED: &str = include_str!("../../../extension-vectors/leko-config-extended.schema.json");

/// The extended schema, parsed once.
pub fn extended_schema() -> &'static Value {
    static SCHEMA: OnceLock<Value> = OnceLock::new();
    SCHEMA.get_or_init(|| serde_json::from_str(EXTENDED).unwrap_or(Value::Null))
}
