mod edit_support;
use edit_support::*;
use edit_runs::{command, fixture, NOW};

#[test]
fn phone_compatibility_warning_matches_the_phone_reader() {
    let (_folder, file) = fixture(None);
    let plain = command(&file, "edit-block", &["--id", "one", "--add-app", "com.three"], NOW);
    assert_eq!(plain.code, 0, "{}", plain.err);
    assert!(!plain.out.contains("Phone Enforcer"));
    let sites = command(&file, "edit-block", &["--id", "one", "--add-site", "example.com"], NOW);
    assert_eq!(sites.code, 0, "{}", sites.err);
    assert!(sites.out.contains("Phone Enforcer 0.3 cannot read"), "{}", sites.out);
}
