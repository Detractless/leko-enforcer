mod scratch_folder;
mod settings_builder;
mod launch_agent_mock;
use leko_cli::launch_agent::agent_command::{run, run_with_backend};
use launch_agent_mock::Mock;
use leko_cli::launch_agent::agent_plist::{render, FILE_NAME, LABEL};
use scratch_folder::Scratch;
use std::ffi::OsString;

#[test]
fn uninstall_only_removes_the_owned_agent() {
    let folder = Scratch::new("agent-uninstall");
    let dir = folder.path().join("Library/LaunchAgents");
    let log = folder.path().canonicalize().unwrap().join("Library/Logs/Leko/agent.log");
    std::fs::create_dir_all(&dir).unwrap();
    let plist = dir.join(FILE_NAME);
    std::fs::write(&plist, render(LABEL, std::path::Path::new("/bin/true"), std::path::Path::new("/tmp/rules"), &log).unwrap()).unwrap();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    assert_eq!(run(&[OsString::from("uninstall")], Some(folder.path().as_os_str()), Some("UTC"), &mut out, &mut err), 0);
    assert!(plist.exists());
    assert!(String::from_utf8(out).unwrap().contains("Nothing was changed"));
}

#[test]
fn settings_and_unrelated_agents_are_unchanged() {
    let folder = Scratch::new("agent-unrelated");
    let settings = settings_builder::write_settings(folder.path(), "settings.zip", &[]);
    let before = std::fs::read(&settings).unwrap();
    let other = folder.path().join("Library/LaunchAgents/other.plist");
    std::fs::create_dir_all(other.parent().unwrap()).unwrap();
    std::fs::write(&other, b"other").unwrap();
    assert_eq!(run(&[OsString::from("uninstall"), OsString::from("--write")], Some(folder.path().as_os_str()), Some("UTC"), &mut Vec::new(), &mut Vec::new()), 1);
    assert_eq!(std::fs::read(settings).unwrap(), before);
    assert_eq!(std::fs::read(other).unwrap(), b"other");
}

#[test]
fn removal_unloads_only_the_owned_service_and_keeps_the_settings() {
    let folder = Scratch::new("agent-removal");
    let home = folder.path().canonicalize().unwrap();
    let settings = settings_builder::write_settings(&home, "settings.zip", &[]);
    let before = std::fs::read(&settings).unwrap();
    let dir = home.join("Library/LaunchAgents");
    let plist = dir.join(FILE_NAME);
    let log = home.join("Library/Logs/Leko/agent.log");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(&plist, render(LABEL, std::path::Path::new("/bin/true"), &settings, &log).unwrap()).unwrap();
    let mut backend = Mock { path: Some(plist.to_string_lossy().into_owned()), pid: Some(42), ..Mock::default() };
    assert_eq!(run_with_backend(&[OsString::from("uninstall"), OsString::from("--write")], Some(home.as_os_str()), Some("UTC"), &mut Vec::new(), &mut Vec::new(), &mut backend), 0);
    assert_eq!(backend.stops, 1);
    assert!(!plist.exists());
    assert_eq!(std::fs::read(settings).unwrap(), before);
}
