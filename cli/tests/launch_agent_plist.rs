use leko_cli::launch_agent::agent_plist::{owned, render, LABEL};
use std::path::Path;
mod scratch_folder;
mod settings_builder;

#[test]
fn plist_preserves_paths_and_agent_settings() {
    let exe = Path::new("/Users/a & b/é <run>");
    let settings = Path::new("/Users/a & b/\"rules\".zip");
    let log = Path::new("/Users/a & b/log");
    let xml = render(LABEL, exe, settings, log).unwrap();
    assert!(xml.contains("a &amp; b/é &lt;run&gt;"));
    assert!(xml.contains("&quot;rules&quot;.zip"));
    assert!(owned(&xml, log));
    assert!(!owned(&xml.replace("<integer>30</integer>", "<integer>1</integer>"), log));
    assert!(render(LABEL, Path::new("relative"), settings, log).is_err());
    let folder = scratch_folder::Scratch::new("agent-plist");
    let plist = folder.path().join("configuration.plist");
    std::fs::write(&plist, &xml).unwrap();
    for (index, expected) in [(0, exe), (2, settings)] {
        let output = std::process::Command::new("/usr/libexec/PlistBuddy").args(["-c", &format!("Print :ProgramArguments:{index}")]).arg(&plist).output().unwrap();
        assert!(output.status.success());
        assert_eq!(String::from_utf8(output.stdout).unwrap().trim_end_matches('\n'), expected.to_str().unwrap());
    }
}

#[test]
fn restart_is_throttled_and_errors_are_visible() {
    let xml = render(LABEL, Path::new("/a"), Path::new("/b"), Path::new("/log")).unwrap();
    assert!(xml.contains("<key>KeepAlive</key><true/>"));
    assert!(xml.contains("<key>ThrottleInterval</key><integer>30</integer>"));
    assert!(xml.contains("<key>StandardErrorPath</key><string>/log</string>"));
}

#[test]
fn invalid_settings_do_not_leak_their_contents_to_agent_errors() {
    let folder = scratch_folder::Scratch::new("agent-log-privacy");
    let secret = "private-rule-sentinel";
    let bad_block = format!(r#"{{"schema":2,"id":"one","name":"B","covers":{{"apps":["com.example.app"]}},"rules":[{{"id":"r","kind":"{secret}"}}],"door":{{"kind":"wall"}}}}"#);
    let bytes = settings_builder::zip_of(&[("GUIDE.md", "guide"), ("general.json", r#"{"schema":2}"#), ("blocks/one.json", &bad_block)]);
    let path = folder.path().join("settings.zip");
    std::fs::write(&path, bytes).unwrap();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_leko")).arg("enforce").arg(path).env("TZ", "UTC").output().unwrap();
    assert!(!output.status.success());
    let err = String::from_utf8(output.stderr).unwrap();
    assert!(err.contains("invalid"));
    assert!(!err.contains(secret));
}
