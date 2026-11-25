use serde::{Deserialize, Serialize};

use crate::error::{CoreError, CoreResult};
use crate::models::{Pass, Penalty, Schedule, Seal, Target};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct TechnologyManagementRule {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub target: Target,
    pub schedule: Schedule,
    #[serde(default)]
    pub penalty: Option<Penalty>,
    #[serde(default)]
    pub pass: Pass,
    #[serde(default)]
    pub seal: Seal,
    pub created_at: String,
    pub updated_at: String,
}

impl TechnologyManagementRule {
    pub fn validate(&self) -> CoreResult<()> {
        if self.target.is_whole_device() && self.penalty.is_some() {
            return Err(CoreError::InvalidInput(
                "a whole device rule cannot carry a penalty".to_string(),
            ));
        }
        if !self.target.is_whole_device() && self.penalty.is_none() {
            return Err(CoreError::InvalidInput(
                "apps and websites rules require a penalty".to_string(),
            ));
        }
        Ok(())
    }
}
