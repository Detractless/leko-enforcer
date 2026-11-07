//! Reads the shared vector files from `vectors/`.
#![allow(dead_code)]

use serde_json::Value;

pub fn vector_path(name: &str) -> String {
    format!("{}/../vectors/{name}", env!("CARGO_MANIFEST_DIR"))
}

pub fn vector_bytes(name: &str) -> Vec<u8> {
    std::fs::read(vector_path(name)).unwrap_or_else(|_| panic!("vectors/{name} is missing"))
}

pub fn read_vector(name: &str) -> Value {
    serde_json::from_slice(&vector_bytes(name)).unwrap_or_else(|_| panic!("vectors/{name} is not JSON"))
}

/// A file Leko Enforcer wrote for its own additions, from `extension-vectors/` (never from the phone's `vectors/`).
pub fn read_extension_vector(name: &str) -> Value {
    let path = format!("{}/../extension-vectors/{name}", env!("CARGO_MANIFEST_DIR"));
    let bytes = std::fs::read(&path).unwrap_or_else(|_| panic!("extension-vectors/{name} is missing"));
    serde_json::from_slice(&bytes).unwrap_or_else(|_| panic!("extension-vectors/{name} is not JSON"))
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
