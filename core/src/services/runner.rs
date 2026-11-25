use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use crate::error::CoreResult;
use crate::io::snapshot::AddonSnapshot;
use crate::services::recipe::{self, Recipe};
use crate::services::verifier::AddonRunner;

#[derive(Clone, Debug)]
pub struct Challenge {
    pub challenge_id: String,
    pub nonce: String,
}

pub struct ProcessRunner {
    pub timeout: Duration,
}

impl Default for ProcessRunner {
    fn default() -> Self {
        ProcessRunner { timeout: Duration::from_secs(5) }
    }
}

fn executable(config: &Value) -> Result<PathBuf, String> {
    let value = config.get("executable").and_then(Value::as_str)
        .ok_or_else(|| "the spec has no executable".to_string())?;
    Ok(PathBuf::from(value))
}

fn accepted(reply: &Value, challenge: &Challenge) -> Result<bool, String> {
    if reply.get("challenge_id").and_then(Value::as_str) != Some(challenge.challenge_id.as_str()) {
        return Err("the challenge id did not match".to_string());
    }
    if reply.get("nonce").and_then(Value::as_str) != Some(challenge.nonce.as_str()) {
        return Err("the nonce did not match".to_string());
    }
    Ok(reply.get("status").and_then(Value::as_str) == Some("satisfied"))
}

impl AddonRunner for ProcessRunner {
    fn run(&self, _snapshot: &AddonSnapshot, config: &Value, challenge: &Challenge)
        -> Result<bool, String> {
        let program = executable(config)?;
        let mut child = Command::new(&program)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|error| error.to_string())?;
        let request = json!({
            "challenge_id": challenge.challenge_id,
            "nonce": challenge.nonce,
            "config": config,
        });
        let mut stdin = child.stdin.take().ok_or_else(|| "no stdin".to_string())?;
        stdin.write_all(request.to_string().as_bytes())
            .map_err(|error| error.to_string())?;
        drop(stdin);
        let mut stdout = child.stdout.take().ok_or_else(|| "no stdout".to_string())?;
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            let mut bytes = Vec::new();
            let _ = stdout.read_to_end(&mut bytes);
            let _ = sender.send(bytes);
        });

        let deadline = Instant::now() + self.timeout;
        let status = loop {
            if let Some(status) = child.try_wait().map_err(|error| error.to_string())? {
                break status;
            }
            if Instant::now() >= deadline {
                let _ = child.kill();
                return Err("the add-on timed out".to_string());
            }
            thread::sleep(Duration::from_millis(5));
        };
        if !status.success() {
            return Err("the add-on exited with a failure".to_string());
        }
        let bytes = receiver.recv_timeout(Duration::from_secs(2))
            .map_err(|_| "no reply was read".to_string())?;
        let reply: Value = serde_json::from_slice(&bytes)
            .map_err(|_| "the reply was not JSON".to_string())?;
        accepted(&reply, challenge)
    }
}

pub struct RecipeRunner {
    pub recipe: Recipe,
}

impl AddonRunner for RecipeRunner {
    fn run(&self, _snapshot: &AddonSnapshot, config: &Value, _challenge: &Challenge)
        -> Result<bool, String> {
        recipe::evaluate(&self.recipe, config).map_err(|error| error.to_string())
    }
}

pub fn parse_recipe(text: &str) -> CoreResult<Recipe> {
    recipe::parse(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;

    fn snapshot() -> AddonSnapshot {
        AddonSnapshot {
            addon_id: "checker".to_string(),
            sha256: "ab".repeat(32),
            bytes: b"code".to_vec(),
        }
    }

    fn script_at(folder: &Path, body: &str) -> PathBuf {
        let path = folder.join("addon.sh");
        fs::write(&path, format!("#!/bin/sh\ncat >/dev/null\necho '{}'\n", body)).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        }
        path
    }

    #[test]
    fn challenge_protocol_requires_exact_fields() {
        let folder = std::env::temp_dir().join(format!("leko-runner-{}", std::process::id()));
        let _ = fs::remove_dir_all(&folder);
        fs::create_dir_all(&folder).unwrap();
        let challenge = Challenge {
            challenge_id: "challenge-1".to_string(),
            nonce: "n".repeat(64),
        };
        let good = json!({"challenge_id":"challenge-1","nonce":"n".repeat(64),"status":"satisfied"});
        let program = script_at(&folder, &good.to_string());
        let config = json!({"executable": program.to_string_lossy()});
        let runner = ProcessRunner::default();
        assert_eq!(runner.run(&snapshot(), &config, &challenge).unwrap(), true);

        let bad = json!({"challenge_id":"other","nonce":"n".repeat(64),"status":"satisfied"});
        let wrong_program = script_at(&folder, &bad.to_string());
        let wrong_config = json!({"executable": wrong_program.to_string_lossy()});
        assert!(runner.run(&snapshot(), &wrong_config, &challenge).is_err());
        fs::remove_dir_all(folder).unwrap();
    }

    #[test]
    fn every_failure_mode_returns_not_satisfied() {
        let folder = std::env::temp_dir().join(format!("leko-runner-fail-{}", std::process::id()));
        let _ = fs::remove_dir_all(&folder);
        fs::create_dir_all(&folder).unwrap();
        let challenge = Challenge {
            challenge_id: "challenge-1".to_string(),
            nonce: "n".repeat(64),
        };
        let runner = ProcessRunner::default();
        assert!(runner.run(&snapshot(), &json!({}), &challenge).is_err());

        let rejected = json!({"challenge_id":"challenge-1","nonce":"n".repeat(64),"status":"rejected"});
        let program = script_at(&folder, &rejected.to_string());
        let config = json!({"executable": program.to_string_lossy()});
        assert_eq!(runner.run(&snapshot(), &config, &challenge).unwrap(), false);

        let malformed = script_at(&folder, "not json");
        let bad_config = json!({"executable": malformed.to_string_lossy()});
        assert!(runner.run(&snapshot(), &bad_config, &challenge).is_err());
        fs::remove_dir_all(folder).unwrap();
    }
}
