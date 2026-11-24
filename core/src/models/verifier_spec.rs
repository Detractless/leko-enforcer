use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct CustomVerifierSpec {
    pub addon_id: String,
    pub sha256: String,
    pub config: serde_json::Value,
}
