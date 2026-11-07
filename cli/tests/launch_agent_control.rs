use leko_cli::launch_agent::agent_control::{current_domain, parse_status, status, Service};

#[test]
fn registration_uses_the_current_gui_domain() {
    let domain = current_domain().unwrap();
    assert!(domain.starts_with("gui/"));
    assert_eq!(status(&domain, "org.leko.enforcer.no-such-test-service").unwrap(), Service::Absent);
}

#[test]
fn status_parser_refuses_incomplete_results() {
    assert_eq!(parse_status("path = /tmp/a\nstate = running\npid = 42\n").unwrap(), Service::Loaded { path: "/tmp/a".into(), pid: Some(42) });
    assert_eq!(parse_status("path = /tmp/a\nstate = waiting\n").unwrap(), Service::Loaded { path: "/tmp/a".into(), pid: None });
    assert!(parse_status("state = running\n").is_err());
    assert!(parse_status("path = /tmp/a\nstate = running\n").is_err());
}
