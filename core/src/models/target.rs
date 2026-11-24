use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DeviceAction {
    Lock,
    Logoff,
    Restart,
    Shutdown,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Target {
    SpecificApps { apps: Vec<String> },
    AllowOnlyApps { allowed_apps: Vec<String> },
    SpecificWebsites { sites: Vec<String> },
    TopicDetectedWebsites { topics: Vec<String>, policy_packs: Vec<String> },
    AllowOnlyWebsites { allowed_sites: Vec<String>, allowed_topics: Vec<String> },
    WholeDevice { action: DeviceAction },
}

impl Target {
    pub fn is_whole_device(&self) -> bool {
        matches!(self, Target::WholeDevice { .. })
    }
}
