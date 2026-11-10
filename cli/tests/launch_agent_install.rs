mod scratch_folder;
mod settings_builder;
mod launch_agent_mock;
use leko_cli::launch_agent::agent_command::{run, run_with_backend};
use launch_agent_mock::Mock;
use scratch_folder::Scratch;
use std::ffi::{OsStr, OsString};

fn words(args: &[&str]) -> Vec<OsString> { args.iter().map(OsString::from).collect() }

#[test]
fn install_previews_and_checks_its_inputs() {
    let folder = Scratch::new("agent-preview");
    let settings = settings_builder::write_settings(folder.path(), "space & rules.zip", &[]);
    let exe = env!("CARGO_BIN_EXE_leko");
    let args = words(&["install", "--executable", exe, "--settings", settings.to_str().unwrap()]);
    let (mut out, mut err) = (Vec::new(), Vec::new());
    assert_eq!(run(&args, Some(folder.path().as_os_str()), Some("UTC"), &mut out, &mut err), 0, "{}", String::from_utf8_lossy(&err));
    assert!(String::from_utf8(out).unwrap().contains("Nothing was changed"));
    assert!(!folder.path().join("Library").exists());
    let bad = words(&["install", "--executable", exe, "--settings", "/no/such/file.zip"]);
    assert_eq!(run(&bad, Some(folder.path().as_os_str()), Some("UTC"), &mut Vec::new(), &mut Vec::new()), 1);
    assert_eq!(run(&args, Some(OsStr::new("/")), None, &mut Vec::new(), &mut Vec::new()), 1);
}

#[test]
fn install_defaults_to_the_managed_settings_file() {
    let folder = Scratch::new("agent-managed-settings");
    let home = folder.path().canonicalize().unwrap();
    let path = leko_cli::settings_store::default_path(Some(home.as_os_str())).unwrap();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    settings_builder::write_settings(path.parent().unwrap(), "settings.zip", &[]);
    let args = words(&["install", "--executable", env!("CARGO_BIN_EXE_leko")]);
    let mut backend = Mock::default();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    assert_eq!(run_with_backend(&args, Some(home.as_os_str()), Some("UTC"), &mut out, &mut err, &mut backend), 0, "{}", String::from_utf8_lossy(&err));
    assert!(String::from_utf8(out).unwrap().contains(path.to_str().unwrap()));
}

#[test]
fn install_is_idempotent_and_refuses_unrelated_files() {
    let folder = Scratch::new("agent-collision");
    let settings = settings_builder::write_settings(folder.path(), "rules.zip", &[]);
    let agent = folder.path().join("Library/LaunchAgents/org.leko.enforcer.agent.plist");
    std::fs::create_dir_all(agent.parent().unwrap()).unwrap();
    std::fs::write(&agent, b"not ours").unwrap();
    let args = words(&["install", "--executable", env!("CARGO_BIN_EXE_leko"), "--settings", settings.to_str().unwrap(), "--write"]);
    assert_eq!(run(&args, Some(folder.path().as_os_str()), Some("UTC"), &mut Vec::new(), &mut Vec::new()), 1);
    assert_eq!(std::fs::read(agent).unwrap(), b"not ours");
}

#[test]
fn repeat_install_and_failed_registration_are_truthful() {
    let folder = Scratch::new("agent-repeat");
    let settings = settings_builder::write_settings(folder.path(), "rules.zip", &[]);
    let args = words(&["install", "--executable", env!("CARGO_BIN_EXE_leko"), "--settings", settings.to_str().unwrap(), "--write"]);
    let mut backend = Mock::default();
    for _ in 0..2 {
        assert_eq!(run_with_backend(&args, Some(folder.path().as_os_str()), Some("UTC"), &mut Vec::new(), &mut Vec::new(), &mut backend), 0);
    }
    assert_eq!(backend.starts, 1);
    let folder = Scratch::new("agent-failed");
    let settings = settings_builder::write_settings(folder.path(), "rules.zip", &[]);
    let args = words(&["install", "--executable", env!("CARGO_BIN_EXE_leko"), "--settings", settings.to_str().unwrap(), "--write"]);
    let mut backend = Mock { fail_start: true, ..Mock::default() };
    let (mut out, mut err) = (Vec::new(), Vec::new());
    assert_eq!(run_with_backend(&args, Some(folder.path().as_os_str()), Some("UTC"), &mut out, &mut err, &mut backend), 1);
    assert!(out.is_empty());
    assert!(String::from_utf8(err).unwrap().contains("refused"));
}
