use std::collections::HashMap;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::thread;

use serde::{Deserialize, Serialize};

use crate::clock::Moment;
use crate::domain::PassState;
use crate::error::{CoreError, CoreResult};
use crate::models::{Pass, PassCap, PassRequirement, TechnologyManagementRule};

pub const PASS_STATE_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PassStateFile {
    pub version: u32,
    pub passes: HashMap<String, StoredPass>,
}

impl Default for PassStateFile {
    fn default() -> Self {
        PassStateFile { version: PASS_STATE_VERSION, passes: HashMap::new() }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StoredPass {
    pub rule_id: String,
    pub requirement: String,
    pub earned_at: String,
    pub window_key: String,
    pub used_in_window: u32,
}

pub trait Sleeper {
    fn sleep(&mut self, millis: u64);

    fn waited_ms(&self) -> u64 {
        0
    }
}

pub struct RealSleeper;

impl Sleeper for RealSleeper {
    fn sleep(&mut self, millis: u64) {
        thread::sleep(std::time::Duration::from_millis(millis));
    }
}

#[derive(Debug, Default)]
pub struct RecordingSleeper {
    pub waited: Vec<u64>,
}

impl Sleeper for RecordingSleeper {
    fn sleep(&mut self, millis: u64) {
        self.waited.push(millis);
    }

    fn waited_ms(&self) -> u64 {
        self.waited.iter().sum()
    }
}

pub fn state_path(store_path: &Path) -> PathBuf {
    let stem = store_path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("rules");
    store_path.with_file_name(format!("{}.pass-state.json", stem))
}

pub fn load(path: &Path) -> CoreResult<PassStateFile> {
    if !path.exists() {
        return Ok(PassStateFile::default());
    }
    let bytes = fs::read(path).map_err(|error| CoreError::Io(error.to_string()))?;
    if bytes.is_empty() {
        return Err(CoreError::Parse("the pass state file is empty".to_string()));
    }
    let state: PassStateFile = serde_json::from_slice(&bytes)
        .map_err(|error| CoreError::Parse(error.to_string()))?;
    if state.version != PASS_STATE_VERSION {
        return Err(CoreError::UnknownVersion(state.version));
    }
    Ok(state)
}

pub fn save(path: &Path, state: &PassStateFile) -> CoreResult<()> {
    if state.version != PASS_STATE_VERSION {
        return Err(CoreError::UnknownVersion(state.version));
    }
    let bytes = serde_json::to_vec_pretty(state)
        .map_err(|error| CoreError::Parse(error.to_string()))?;
    let temp = path.with_extension("tmp");
    let mut file = File::create(&temp).map_err(|error| CoreError::Io(error.to_string()))?;
    file.write_all(&bytes)
        .map_err(|error| CoreError::Io(error.to_string()))?;
    file.sync_all().map_err(|error| CoreError::Io(error.to_string()))?;
    drop(file);
    fs::rename(&temp, path).map_err(|error| CoreError::Io(error.to_string()))?;
    Ok(())
}

pub fn domain_state(state: &PassStateFile, rule_id: &str) -> PassState {
    match state.passes.get(rule_id) {
        Some(pass) => PassState {
            earned_at: Some(pass.earned_at.clone()),
            used_in_window: pass.used_in_window,
            window_key: pass.window_key.clone(),
        },
        None => PassState {
            earned_at: None,
            used_in_window: 0,
            window_key: String::new(),
        },
    }
}

pub fn wait_for_requirement(requirement: &PassRequirement, sleeper: &mut dyn Sleeper)
    -> CoreResult<()> {
    if let PassRequirement::CountdownPass { wait_seconds } = requirement {
        sleeper.sleep(wait_seconds.saturating_mul(1000));
    }
    Ok(())
}

pub fn record(
    state: &mut PassStateFile,
    rule: &TechnologyManagementRule,
    moment: &Moment,
    window_key: String,
) -> CoreResult<()> {
    let Pass::EarnPass { requirement, cap, .. } = &rule.pass else {
        return Err(CoreError::InvalidInput("this rule has no pass".to_string()));
    };
    let previous = state.passes.get(&rule.id);
    let used = match previous {
        Some(pass) if pass.window_key == window_key => pass.used_in_window,
        _ => 0,
    };
    if let PassCap::LimitedPasses { max_per_window } = cap {
        if used >= *max_per_window {
            return Err(CoreError::InvalidInput("the pass cap was reached".to_string()));
        }
    }
    state.passes.insert(
        rule.id.clone(),
        StoredPass {
            rule_id: rule.id.clone(),
            requirement: format!("{:?}", requirement),
            earned_at: moment.utc.to_rfc3339(),
            window_key,
            used_in_window: used + 1,
        },
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn countdown_wait_is_enforced_with_an_injected_sleeper() {
        let mut sleeper = RecordingSleeper::default();
        let requirement = PassRequirement::CountdownPass { wait_seconds: 5 };
        assert!(wait_for_requirement(&requirement, &mut sleeper).is_ok());
        assert_eq!(sleeper.waited, vec![5000]);
        let mut immediate = RecordingSleeper::default();
        assert!(wait_for_requirement(&PassRequirement::PhysicalPass, &mut immediate).is_ok());
        assert!(immediate.waited.is_empty());
    }
}
