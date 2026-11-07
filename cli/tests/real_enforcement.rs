#![cfg(target_os = "macos")]
mod scratch_folder;
mod settings_builder;
use leko_cli::enforcement::enforcement_loop::judge_apps;
use leko_cli::enforcement::macos_applications::{running, terminate, Application};
use leko_core::config_time::zone_time::ZoneTime;
use leko_core::models::specs::{AppCover, BlockSpec, Door, Lock, ReadSet, Rule};
use scratch_folder::Scratch;
use std::process::Command;
use std::time::{Duration, Instant};

struct TestApp(Application);
impl Drop for TestApp { fn drop(&mut self) { let _ = terminate(&self.0); } }
struct Enforcer(std::process::Child);
impl Drop for Enforcer { fn drop(&mut self) { let _ = self.0.kill(); let _ = self.0.wait(); } }
extern "C" { fn kill(pid: i32, signal: i32) -> i32; }
fn alive(pid: i32) -> bool { unsafe { kill(pid, 0) == 0 } }

#[test]
fn real_mac_scheduled_app_termination() {
    let folder = Scratch::new("real-enforcement");
    let contents = folder.path().join("Disposable.app/Contents");
    std::fs::create_dir_all(contents.join("MacOS")).unwrap();
    std::fs::write(contents.join("Info.plist"), r#"<?xml version="1.0" encoding="UTF-8"?><!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd"><plist version="1.0"><dict><key>CFBundleIdentifier</key><string>org.leko.disposable-enforcement-test</string><key>CFBundleExecutable</key><string>disposable</string><key>CFBundlePackageType</key><string>APPL</string></dict></plist>"#).unwrap();
    let source = folder.path().join("disposable.swift");
    std::fs::write(&source, "import AppKit\nlet app = NSApplication.shared\napp.setActivationPolicy(.accessory)\napp.run()\n").unwrap();
    let executable = contents.join("MacOS/disposable");
    assert!(Command::new("/usr/bin/swiftc").args([source.as_os_str(), "-o".as_ref(), executable.as_os_str()]).status().unwrap().success());
    let file = settings_builder::write_settings(folder.path(), "settings.zip", &[]);
    let mut enforcer = Enforcer(Command::new(env!("CARGO_BIN_EXE_leko")).arg("enforce").arg(&file).env("TZ", "UTC").stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).spawn().unwrap());
    std::thread::sleep(Duration::from_secs(2));
    assert!(Command::new("/usr/bin/open").arg("-n").arg(contents.parent().unwrap()).status().unwrap().success());
    let deadline = Instant::now() + Duration::from_secs(10);
    let app = loop {
        if let Some(app) = running().unwrap().into_iter().find(|app| app.bundle.as_deref() == Some("org.leko.disposable-enforcement-test")) { break app; }
        assert!(Instant::now() < deadline, "test app was not registered with AppKit");
        std::thread::sleep(Duration::from_millis(100));
    };
    let child = TestApp(app.clone());
    let mut set = ReadSet::default();
    set.blocks.insert("test".into(), BlockSpec { id: "test".into(), on: true, apps: AppCover::Named(vec![app.bundle.clone().unwrap()]), sites: vec![], rules: vec![Rule { id: "session".into(), kind: "session".into(), days: vec![], allowed: vec![], ends_at: Some(1_700_000_001_000) }], door: Door::Wall, pass: None, earn: None, lock: Lock::open() });
    let zone = ZoneTime::new("UTC").unwrap();
    let mut errors = Vec::new();
    judge_apps(&set, &[app.clone()], 1_700_000_001_000, &zone, std::process::id() as i32, terminate, &mut errors);
    assert!(alive(child.0.pid), "open session terminated the app");
    std::thread::sleep(Duration::from_secs(2));
    assert!(enforcer.0.try_wait().unwrap().is_none(), "enforcement exited unexpectedly");
    assert!(alive(child.0.pid), "empty settings terminated the app");
    if std::env::var_os("LEKO_SLEEP_BOUNDARY_CHECK").is_some() {
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
        let start_minute = (now / 60) % 1440;
        let end_minute = (start_minute + 2) % 1440;
        let from = format!("{:02}:{:02}", start_minute / 60, start_minute % 60);
        let to = format!("{:02}:{:02}", end_minute / 60, end_minute % 60);
        let boundary = (now / 60 + 2) * 60;
        let boundary_local = String::from_utf8(Command::new("/bin/date").args(["-r", &boundary.to_string(), "+%Y-%m-%d %H:%M:%S"]).output().unwrap().stdout).unwrap().trim().to_string();
        settings_builder::write_settings(folder.path(), "settings.zip", &[settings_builder::block("test", serde_json::json!({"apps": [app.bundle.clone().unwrap()]}), serde_json::json!([settings_builder::outside_hours("hours", &[], &[(&from, &to)])]))]);
        std::thread::sleep(Duration::from_secs(3));
        assert!(alive(child.0.pid), "app closed before its scheduled boundary");
        let started_local = String::from_utf8(Command::new("/bin/date").arg("+%Y-%m-%d %H:%M:%S").output().unwrap().stdout).unwrap().trim().to_string();
        println!("Putting the Mac to sleep. Wake it after two and a half minutes.");
        assert!(Command::new("/usr/bin/pmset").arg("sleepnow").status().unwrap().success());
        std::thread::sleep(Duration::from_secs(30));
        let log = Command::new("/usr/bin/pmset").args(["-g", "log"]).output().unwrap();
        assert!(log.status.success());
        let log = String::from_utf8_lossy(&log.stdout);
        let slept_before = log.lines().any(|line| line.get(..19).is_some_and(|date| date >= started_local.as_str() && date < boundary_local.as_str()) && line.contains(" Sleep ") && line.contains("Entering Sleep"));
        let woke_after = log.lines().any(|line| line.get(..19).is_some_and(|date| date >= boundary_local.as_str()) && (line.contains(" Wake ") || line.contains(" DarkWake ")));
        assert!(slept_before && woke_after, "power log did not establish sleep across the boundary {boundary_local}");
        assert!(enforcer.0.try_wait().unwrap().is_none(), "enforcement exited during sleep");
        let deadline = Instant::now() + Duration::from_secs(15);
        while alive(child.0.pid) {
            assert!(Instant::now() < deadline, "app did not close after the sleeping boundary");
            std::thread::sleep(Duration::from_millis(100));
        }
        return;
    }
    if std::env::var_os("LEKO_REAL_SLEEP_CHECK").is_some() {
        println!("Putting the Mac to sleep. Wake it after at least 15 seconds.");
        let started = String::from_utf8(Command::new("/bin/date").arg("+%Y-%m-%d %H:%M:%S").output().unwrap().stdout).unwrap().trim().to_string();
        assert!(Command::new("/usr/bin/pmset").arg("sleepnow").status().unwrap().success(), "could not request sleep");
        std::thread::sleep(Duration::from_secs(30));
        let log = Command::new("/usr/bin/pmset").args(["-g", "log"]).output().unwrap();
        assert!(log.status.success(), "could not inspect the macOS power log");
        let log = String::from_utf8_lossy(&log.stdout);
        assert!(log.lines().any(|line| line.get(..19).is_some_and(|date| date >= started.as_str()) && line.contains(" Sleep ")), "no sleep event after {started} in the power log");
        assert!(enforcer.0.try_wait().unwrap().is_none(), "enforcement exited during sleep");
        assert!(alive(child.0.pid), "the disposable app exited during sleep");
    }
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as i64;
    let date = leko_core::sessions::utc_moment::format_utc_moment(now + 60_000).unwrap();
    settings_builder::write_settings(folder.path(), "settings.zip", &[settings_builder::block("test", serde_json::json!({"apps": [app.bundle.unwrap()]}), serde_json::json!([settings_builder::session("session", &date)]))]);
    let deadline = Instant::now() + Duration::from_secs(10);
    while alive(child.0.pid) {
        assert!(Instant::now() < deadline, "closed session did not terminate app: {}", String::from_utf8_lossy(&errors));
        std::thread::sleep(Duration::from_millis(100));
    }
}
