mod scratch_folder;
mod settings_builder;

use leko_cli::settings_store;
use scratch_folder::{run_leko, Scratch};
use std::path::PathBuf;

fn target(home: &Scratch) -> PathBuf {
    settings_store::default_path(Some(home.path().as_os_str())).unwrap()
}

#[test]
fn init_creates_valid_private_settings_and_never_replaces_them() {
    let home = Scratch::new("settings-init");
    let temp = Scratch::new("settings-init-tmp");
    let path = target(&home);
    let first = run_leko(&home, &temp, &["settings", "init"]);
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    assert!(path.is_file());
    assert!(leko_cli::status::settings_file::read_settings(path.to_str().unwrap()).is_ok());

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(
            std::fs::metadata(path.parent().unwrap())
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
    }

    let before = std::fs::read(&path).unwrap();
    let second = run_leko(&home, &temp, &["settings", "init"]);
    assert!(
        second.status.success(),
        "{}",
        String::from_utf8_lossy(&second.stderr)
    );
    assert_eq!(std::fs::read(path).unwrap(), before);
}

#[test]
fn import_validates_and_copies_an_export_without_overwriting() {
    let home = Scratch::new("settings-import");
    let temp = Scratch::new("settings-import-tmp");
    let source = settings_builder::write_settings(home.path(), "export.zip", &[]);
    let path = target(&home);
    let imported = run_leko(
        &home,
        &temp,
        &["settings", "import", source.to_str().unwrap()],
    );
    assert!(
        imported.status.success(),
        "{}",
        String::from_utf8_lossy(&imported.stderr)
    );
    let original = std::fs::read(&source).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), original);

    let other = settings_builder::write_settings(home.path(), "other.zip", &[]);
    let refused = run_leko(
        &home,
        &temp,
        &["settings", "import", other.to_str().unwrap()],
    );
    assert!(!refused.status.success());
    assert!(String::from_utf8_lossy(&refused.stderr).contains("left unchanged"));
    assert_eq!(std::fs::read(&path).unwrap(), original);
}

#[test]
fn import_migrates_pending_changes_with_the_settings() {
    let home = Scratch::new("settings-waiting-import");
    let temp = Scratch::new("settings-waiting-import-tmp");
    let source = settings_builder::write_settings(home.path(), "export.zip", &[]);
    let waiting = format!("{}.waiting.json", source.display());
    std::fs::write(&waiting, b"{\"format\":1,\"changes\":[]}\n").unwrap();
    let path = target(&home);
    let imported = run_leko(
        &home,
        &temp,
        &["settings", "import", source.to_str().unwrap()],
    );
    assert!(
        imported.status.success(),
        "{}",
        String::from_utf8_lossy(&imported.stderr)
    );
    assert_eq!(
        std::fs::read(format!("{}.waiting.json", path.display())).unwrap(),
        std::fs::read(waiting).unwrap()
    );
}

#[test]
fn invalid_import_does_not_create_or_replace_settings() {
    let home = Scratch::new("settings-invalid");
    let temp = Scratch::new("settings-invalid-tmp");
    let source = home.path().join("bad.zip");
    std::fs::write(&source, b"not an export").unwrap();
    let result = run_leko(
        &home,
        &temp,
        &["settings", "import", source.to_str().unwrap()],
    );
    assert!(!result.status.success());
    assert!(!target(&home).exists());
}

#[test]
fn validation_reports_a_broken_pending_changes_file() {
    let home = Scratch::new("settings-bad-waiting");
    let temp = Scratch::new("settings-bad-waiting-tmp");
    let path = settings_builder::write_settings(home.path(), "current.zip", &[]);
    std::fs::write(format!("{}.waiting.json", path.display()), b"not json").unwrap();
    let result = run_leko(
        &home,
        &temp,
        &["settings", "validate", path.to_str().unwrap()],
    );
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("waiting file"));
}

#[test]
fn path_command_reports_the_default_user_store() {
    let home = Scratch::new("settings-path");
    let temp = Scratch::new("settings-path-tmp");
    let result = run_leko(&home, &temp, &["settings", "path"]);
    assert!(result.status.success());
    assert_eq!(
        String::from_utf8(result.stdout).unwrap().trim(),
        target(&home).to_str().unwrap()
    );
}
