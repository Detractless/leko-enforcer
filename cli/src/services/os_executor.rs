use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
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

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ForegroundActivity {
    pub bundle_id: Option<String>,
    pub site: Option<String>,
    pub visible_apps: Vec<String>,
}

pub trait ForegroundObserver {
    fn observe_foreground(&mut self) -> Result<Option<ForegroundActivity>, String>;
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

    fn observe_foreground(&mut self) -> Result<Option<ForegroundActivity>, String> {
        Ok(None)
    }

    fn tick_moment_override(&mut self) -> Option<String> {
        None
    }
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
    pub foreground: Option<ForegroundActivity>,
    pub foreground_sequence: Vec<Option<ForegroundActivity>>,
    pub foreground_cursor: usize,
    pub clock_ticks: Vec<String>,
    pub tick_cursor: usize,
}

impl ForegroundObserver for MockExecutor {
    fn observe_foreground(&mut self) -> Result<Option<ForegroundActivity>, String> {
        if !self.foreground_sequence.is_empty() {
            let idx = self.foreground_cursor.min(self.foreground_sequence.len() - 1);
            self.foreground_cursor += 1;
            return Ok(self.foreground_sequence[idx].clone());
        }
        Ok(self.foreground.clone())
    }
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

    fn observe_foreground(&mut self) -> Result<Option<ForegroundActivity>, String> {
        ForegroundObserver::observe_foreground(self)
    }

    fn tick_moment_override(&mut self) -> Option<String> {
        if self.clock_ticks.is_empty() {
            return None;
        }
        let idx = self.tick_cursor.min(self.clock_ticks.len() - 1);
        self.tick_cursor += 1;
        Some(self.clock_ticks[idx].clone())
    }
}

pub struct SystemExecutor {
    shutdown: Arc<AtomicBool>,
    heartbeats_path: Option<PathBuf>,
    eval_log_path: Option<PathBuf>,
}

const BROWSER_BUNDLES: &[&str] = &[
    "com.apple.Safari",
    "com.google.Chrome",
    "org.mozilla.firefox",
    "com.brave.Browser",
    "company.thebrowser.Browser",
    "com.microsoft.edgemac",
    "com.operasoftware.Opera",
];

pub fn observe_with_frontmost<F, V>(
    frontmost_fn: F,
    visible_fn: V,
    heartbeats_path: Option<&Path>,
    eval_log_path: Option<&Path>,
) -> Result<Option<ForegroundActivity>, String>
where
    F: FnOnce() -> Result<Option<macos::Application>, String>,
    V: FnOnce() -> Result<Vec<String>, String>,
{
    let bundle_id = frontmost_fn()?.and_then(|app| app.bundle).filter(|b| !b.is_empty());
    let visible_apps = visible_fn().unwrap_or_default();
    if bundle_id.is_none() && visible_apps.is_empty() {
        return Ok(None);
    }
    let is_browser = bundle_id
        .as_deref()
        .is_some_and(|id| BROWSER_BUNDLES.iter().any(|k| k.eq_ignore_ascii_case(id)));
    let mut site = None;
    if let (Some(bid), Some(path)) = (bundle_id.as_deref(), heartbeats_path.filter(|p| p.exists())) {
        if let Ok(bytes) = fs::read(path) {
            site = crate::services::usage_tracker::parse_site_from_heartbeats(&bytes, bid);
        }
    }
    if site.is_none() && is_browser {
        if let Some(path) = eval_log_path.filter(|p| p.exists()) {
            if let Ok(text) = fs::read_to_string(path) {
                site = crate::services::usage_tracker::parse_site_from_eval_log(&text);
            }
        }
    }
    Ok(Some(ForegroundActivity {
        bundle_id,
        site,
        visible_apps,
    }))
}

impl SystemExecutor {
    pub fn new(heartbeats_path: Option<PathBuf>) -> Result<Self, String> {
        let eval_log_path = std::env::var_os("WAE_RUNTIME_DIR")
            .map(PathBuf::from)
            .map(|dir| dir.join("evaluations.jsonl"));
        Self::new_with_sources(heartbeats_path, eval_log_path)
    }

    pub fn new_with_sources(
        heartbeats_path: Option<PathBuf>,
        eval_log_path: Option<PathBuf>,
    ) -> Result<Self, String> {
        let shutdown = Arc::new(AtomicBool::new(false));
        let copy = shutdown.clone();
        let _ = ctrlc::set_handler(move || copy.store(true, Ordering::SeqCst));
        Ok(SystemExecutor {
            shutdown,
            heartbeats_path,
            eval_log_path,
        })
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

impl ForegroundObserver for SystemExecutor {
    fn observe_foreground(&mut self) -> Result<Option<ForegroundActivity>, String> {
        observe_with_frontmost(
            macos::frontmost,
            macos::visible_apps,
            self.heartbeats_path.as_deref(),
            self.eval_log_path.as_deref(),
        )
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

    fn observe_foreground(&mut self) -> Result<Option<ForegroundActivity>, String> {
        ForegroundObserver::observe_foreground(self)
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

    #[test]
    fn system_observer_parses_frontmost_and_browser_site() {
        let dir = std::env::temp_dir().join(format!("leko-obs-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let hb = dir.join("heartbeats.json");
        fs::write(&hb, r#"{"com.apple.Safari":{"active_site":"https://www.Reddit.com/r/rust"}}"#).unwrap();
        let eval = dir.join("evaluations.jsonl");
        fs::write(&eval, "{\"url\":\"https://news.ycombinator.com/item?id=1\"}\n").unwrap();

        let from_hb = observe_with_frontmost(
            || Ok(Some(macos::Application { pid: 9, bundle: Some("com.apple.Safari".into()) })),
            || Ok(vec!["com.apple.Notes".into()]),
            Some(&hb),
            Some(&eval),
        ).unwrap().unwrap();
        assert_eq!(from_hb.bundle_id.as_deref(), Some("com.apple.Safari"));
        assert_eq!(from_hb.site.as_deref(), Some("www.reddit.com"));
        assert_eq!(from_hb.visible_apps, vec!["com.apple.Notes".to_string()]);

        let from_eval = observe_with_frontmost(
            || Ok(Some(macos::Application { pid: 10, bundle: Some("com.google.Chrome".into()) })),
            || Ok(Vec::new()),
            Some(&hb),
            Some(&eval),
        ).unwrap().unwrap();
        assert_eq!(from_eval.site.as_deref(), Some("news.ycombinator.com"));

        let non_browser = observe_with_frontmost(
            || Ok(Some(macos::Application { pid: 11, bundle: Some("com.apple.TextEdit".into()) })),
            || Ok(Vec::new()),
            Some(&hb),
            Some(&eval),
        ).unwrap().unwrap();
        assert_eq!(non_browser.bundle_id.as_deref(), Some("com.apple.TextEdit"));
        assert_eq!(non_browser.site, None);
        fs::remove_dir_all(dir).unwrap();
    }
}
