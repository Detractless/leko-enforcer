use leko_core::domain::Decision;
use leko_core::TechnologyManagementRule;
use serde_json::{json, Value};

pub fn rule(rule: &TechnologyManagementRule) -> Value {
    serde_json::to_value(rule).expect("a native rule always serializes")
}

pub fn rules(rules: &[TechnologyManagementRule]) -> Value {
    json!({ "rules": rules })
}

pub fn decision(decision: &Decision) -> Value {
    json!({
        "enforced": decision.enforced,
        "reason": format!("{:?}", decision.reason),
    })
}

pub fn message(text: &str) -> Value {
    json!({ "message": text })
}

pub fn status(name: &str, value: &str) -> Value {
    json!({ "status": name, "value": value })
}
