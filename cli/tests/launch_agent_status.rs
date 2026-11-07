mod scratch_folder;
mod launch_agent_mock;
use leko_cli::launch_agent::agent_command::{run, run_with_backend};
use launch_agent_mock::Mock;
use scratch_folder::Scratch;
use std::ffi::OsString;

#[test]
fn status_distinguishes_installed_loaded_and_running() {
    let folder = Scratch::new("agent-status");
    let (mut out, mut err) = (Vec::new(), Vec::new());
    assert_eq!(run(&[OsString::from("status")], Some(folder.path().as_os_str()), Some("UTC"), &mut out, &mut err), 0, "{}", String::from_utf8_lossy(&err));
    assert_eq!(String::from_utf8(out).unwrap(), "Installed: false; loaded: false; running: false.\n");
}

#[test]
fn reports_running_and_stopped_and_rejects_a_different_service_path() {
    let folder = Scratch::new("agent-states");
    let home = folder.path().canonicalize().unwrap();
    let plist = home.join("Library/LaunchAgents/org.leko.enforcer.agent.plist");
    let mut backend = Mock { path: Some(plist.to_string_lossy().into_owned()), ..Mock::default() };
    let mut out = Vec::new();
    let args = [OsString::from("status")];
    assert_eq!(run_with_backend(&args, Some(home.as_os_str()), Some("UTC"), &mut out, &mut Vec::new(), &mut backend), 0);
    assert!(String::from_utf8(out).unwrap().contains("loaded: true; running: false"));
    backend.pid = Some(42);
    let mut out = Vec::new();
    assert_eq!(run_with_backend(&args, Some(home.as_os_str()), Some("UTC"), &mut out, &mut Vec::new(), &mut backend), 0);
    assert!(String::from_utf8(out).unwrap().contains("loaded: true; running: true"));
    backend.path = Some("/unrelated/service.plist".into());
    assert_eq!(run_with_backend(&args, Some(home.as_os_str()), Some("UTC"), &mut Vec::new(), &mut Vec::new(), &mut backend), 1);
}
