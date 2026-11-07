//! Equality of JSON values as the phone's judge reads it: objects are the same whatever the order
//! of their keys, arrays only in the same order, numbers by value (1 and 1.0 are one number), and a
//! missing value is not the same as a null. Ported from `lib/json-same.js`.

use serde_json::Value;

/// True when `a` and `b` are the same JSON value.
pub fn same_json(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => x.as_f64() == y.as_f64(),
        (Value::Array(x), Value::Array(y)) => x.len() == y.len() && x.iter().zip(y).all(|(p, q)| same_json(p, q)),
        (Value::Object(x), Value::Object(y)) => x.len() == y.len() && x.iter().all(|(key, p)| y.get(key).is_some_and(|q| same_json(p, q))),
        _ => a == b,
    }
}

/// Like `same_json`, where `None` is a value that is not there: it equals only another `None`.
pub fn same_json_opt(a: Option<&Value>, b: Option<&Value>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(x), Some(y)) => same_json(x, y),
        _ => false,
    }
}
