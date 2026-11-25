use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;

use leko_core::enforcement::{BrowserId, Heartbeat};
use leko_core::models::DeviceAction;
use leko_platform_macos as macos;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ObservedApp {
    pub pid: i32,
    pub bundle: Option<String>,
}

pub trait OsExecutor {
    fn running_apps(&self) -> Result<Vec<ObservedApp>, String>;
    fn terminate_app(&mut self, app: &ObservedApp) -> Result<(), String>;
    fn browsers(&self) -> Result<Vec<BrowserId>, String>;
    fn heartbeats(&self) -> HashMap<BrowserId, Heartbeat>;
    fn warn_browser(&mut self, browser: &BrowserId);
    fn close_browser(&mut self, browser: &BrowserId);
    fn device_action(&mut self, action: DeviceAction) -> Result<(), String>;
    fn sleep(&self, millis: u64);
    fn shutdown_requested(&self) -> bool;
}

#[derive(Debug, Default)]
pub struct MockExecutor {
    pub apps: Vec<ObservedApp>,
    pub browsers: Vec<BrowserId>,
    pub beats: HashMap<BrowserId, Heartbeat>,
    pub terminated: Vec<String>,
    pub warned: Vec<String>,
    pub closed: Vec<String>,
    pub devices: Vec<DeviceAction>,
    pub sleeps: usize,
    pub shutdown: bool,
    pub fail_device: bool,
}

impl OsExecutor for MockExecutor {
    fn running_apps(&self) -> Result<Vec<ObservedApp>, String> {
        Ok(self.apps.clone())
    }

    fn terminate_app(&mut self, app: &ObservedApp) -> Result<(), String> {
        self.terminated.push(app.bundle.clone().unwrap_or_default());
        Ok(())
    }

    fn browsers(&self) -> Result<Vec<BrowserId>, String> {
        Ok(self.browsers.clone())
    }

    fn heartbeats(&self) -> HashMap<BrowserId, Heartbeat> {
        self.beats.clone()
    }

    fn warn_browser(&mut self, browser: &BrowserId) {
        self.warned.push(browser.bundle_id.clone());
    }

    fn close_browser(&mut self, browser: &BrowserId) {
        self.closed.push(browser.bundle_id.clone());
    }

    fn device_action(&mut self, action: DeviceAction) -> Result<(), String> {
        if self.fail_device {
            return Err("the device action is unavailable".to_string());
        }
        self.devices.push(action);
        Ok(())
    }

    fn sleep(&self, _millis: u64) {}

    fn shutdown_requested(&self) -> bool {
        self.shutdown
    }
}

pub struct SystemExecutor {
    shutdown: Arc<AtomicBool>,
    heartbeats_path: Option<PathBuf>,
}

impl SystemExecutor {
    pub fn new(heartbeats_path: Option<PathBuf>) -> Result<Self, String> {
        let shutdown = Arc::new(AtomicBool::new(false));
        let copy = shutdown.clone();
        let _ = ctrlc::set_handler(move || copy.store(true, Ordering::SeqCst));
        Ok(SystemExecutor { shutdown, heartbeats_path })
    }

    fn load_heartbeats(&self) -> Result<HashMap<BrowserId, Heartbeat>, String> {
        let Some(path) = &self.heartbeats_path else {
            return Ok(HashMap::new());
        };
        if !path.exists() {
            return Ok(HashMap::new());
        }
        let bytes = fs::read(path).map_err(|error| error.to_string())?;
        serde_json::from_slice(&bytes).map_err(|error| error.to_string())
    }
}

impl OsExecutor for SystemExecutor {
    fn running_apps(&self) -> Result<Vec<ObservedApp>, String> {
        let found = macos::running()?;
        Ok(found
            .into_iter()
            .map(|app| ObservedApp { pid: app.pid, bundle: app.bundle })
            .collect())
    }

    fn terminate_app(&mut self, app: &ObservedApp) -> Result<(), String> {
        let target = macos::Application { pid: app.pid, bundle: app.bundle.clone() };
        macos::terminate(&target)
    }

    fn browsers(&self) -> Result<Vec<BrowserId>, String> {
        Ok(self.load_heartbeats()?.keys().cloned().collect())
    }

    fn heartbeats(&self) -> HashMap<BrowserId, Heartbeat> {
        self.load_heartbeats().unwrap_or_default()
    }

    fn warn_browser(&mut self, browser: &BrowserId) {
        let _ = writeln!(std::io::stderr(), "Browser {} has no healthy extension.", browser.bundle_id);
    }

    fn close_browser(&mut self, browser: &BrowserId) {
        let target = macos::Application {
            pid: browser.pid as i32,
            bundle: Some(browser.bundle_id.clone()),
        };
        let _ = macos::terminate(&target);
    }

    fn device_action(&mut self, action: DeviceAction) -> Result<(), String> {
        macos::perform_device_action(action)
    }

    fn sleep(&self, millis: u64) {
        thread::sleep(std::time::Duration::from_millis(millis));
    }

    fn shutdown_requested(&self) -> bool {
        self.shutdown.load(Ordering::SeqCst)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_executors_never_touch_the_real_machine() {
        let mut mock = MockExecutor::default();
        let app = ObservedApp { pid: 4242, bundle: Some("com.example.Block".to_string()) };
        mock.apps.push(app.clone());
        assert_eq!(mock.running_apps().unwrap().len(), 1);
        assert!(mock.terminate_app(&app).is_ok());
        assert_eq!(mock.terminated, vec!["com.example.Block".to_string()]);
        assert!(mock.device_action(DeviceAction::Lock).is_ok());
        assert_eq!(mock.devices, vec![DeviceAction::Lock]);
        assert!(!mock.shutdown_requested());
    }
}
