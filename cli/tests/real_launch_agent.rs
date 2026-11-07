#![cfg(target_os = "macos")]
mod scratch_folder;
mod settings_builder;
use leko_cli::launch_agent::agent_control::{bootstrap, bootout, current_domain, status, Service};
use leko_cli::launch_agent::agent_plist::render;
use leko_cli::enforcement::macos_applications::{running, terminate, Application};
use scratch_folder::Scratch;
use std::process::Command;
use std::time::{Duration, Instant};

struct Agent { domain: String, label: String }
impl Drop for Agent { fn drop(&mut self) { let _ = bootout(&self.domain, &self.label); } }
struct TestApp(Application);
impl Drop for TestApp { fn drop(&mut self) { let _ = terminate(&self.0); } }
extern "C" { fn kill(pid: i32, signal: i32) -> i32; }
fn alive(pid: i32) -> bool { unsafe { kill(pid, 0) == 0 } }

#[test]
fn disposable_agent_starts_enforcement_and_is_removed() {
    let folder = Scratch::new("real-launch-agent");
    let base = folder.path().canonicalize().unwrap();
    let contents = base.join("Disposable.app/Contents");
    std::fs::create_dir_all(contents.join("MacOS")).unwrap();
    let id = format!("org.leko.test.app.{}", std::process::id());
    let info = format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?><!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\"><plist version=\"1.0\"><dict><key>CFBundleIdentifier</key><string>{id}</string><key>CFBundleExecutable</key><string>disposable</string><key>CFBundlePackageType</key><string>APPL</string></dict></plist>");
    std::fs::write(contents.join("Info.plist"), info).unwrap();
    let source = base.join("disposable.swift");
    std::fs::write(&source, "import AppKit\nlet app = NSApplication.shared\napp.setActivationPolicy(.accessory)\napp.run()\n").unwrap();
    assert!(Command::new("/usr/bin/swiftc").args([source.as_os_str(), "-o".as_ref(), contents.join("MacOS/disposable").as_os_str()]).status().unwrap().success());
    let settings = settings_builder::write_settings(&base, "rules.zip", &[]);
    let domain = current_domain().unwrap();
    let label = format!("org.leko.enforcer.test.{}", std::process::id());
    assert_eq!(status(&domain, &label).unwrap(), Service::Absent);
    let plist = base.join("test-agent.plist");
    let log = base.join("agent.log");
    std::fs::write(&plist, render(&label, std::path::Path::new(env!("CARGO_BIN_EXE_leko")), &settings, &log).unwrap()).unwrap();
    bootstrap(&domain, &plist).unwrap();
    let guard = Agent { domain: domain.clone(), label: label.clone() };
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        if matches!(status(&domain, &label), Ok(Service::Loaded { pid: Some(_), .. })) { break; }
        assert!(Instant::now() < deadline, "agent did not start: {:?}", std::fs::read_to_string(&log));
        std::thread::sleep(Duration::from_millis(100));
    }
    assert!(Command::new("/usr/bin/open").arg("-n").arg(contents.parent().unwrap()).status().unwrap().success());
    let deadline = Instant::now() + Duration::from_secs(10);
    let app = loop {
        if let Some(app) = running().unwrap().into_iter().find(|app| app.bundle.as_deref() == Some(&id)) { break app; }
        assert!(Instant::now() < deadline, "test app was not registered");
        std::thread::sleep(Duration::from_millis(50));
    };
    let app = TestApp(app);
    settings_builder::write_settings(&base, "rules.zip", &[settings_builder::block("test", serde_json::json!({"apps": [id]}), serde_json::json!([settings_builder::always("a", &[])]))]);
    let deadline = Instant::now() + Duration::from_secs(15);
    while alive(app.0.pid) {
        assert!(Instant::now() < deadline, "agent did not close the app: {:?}", std::fs::read_to_string(&log));
        std::thread::sleep(Duration::from_millis(100));
    }
    bootout(&domain, &label).unwrap();
    assert_eq!(status(&domain, &label).unwrap(), Service::Absent);
    drop(guard);
}
