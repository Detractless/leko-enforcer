use serde::{Deserialize, Serialize};

use crate::error::{CoreError, CoreResult};
use crate::models::TechnologyManagementRule;
use crate::store::version::{check_version, CURRENT_VERSION};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct NativeStore {
    pub version: u32,
    pub rules: Vec<TechnologyManagementRule>,
}

impl NativeStore {
    pub fn empty() -> Self {
        NativeStore {
            version: CURRENT_VERSION,
            rules: Vec::new(),
        }
    }

    pub fn from_bytes(bytes: &[u8]) -> CoreResult<Self> {
        let store: NativeStore =
            serde_json::from_slice(bytes).map_err(|error| CoreError::Parse(error.to_string()))?;
        check_version(store.version)?;
        for rule in &store.rules {
            rule.validate()?;
        }
        Ok(store)
    }

    pub fn to_bytes(&self) -> CoreResult<Vec<u8>> {
        check_version(self.version)?;
        for rule in &self.rules {
            rule.validate()?;
        }
        serde_json::to_vec_pretty(self).map_err(|error| CoreError::Parse(error.to_string()))
    }
}
