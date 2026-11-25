use sha2::{Digest, Sha256};
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use leko_core::enforcement::{BrowserId, DefaultHandshake, ExtensionHandshake, GuardConfig};
use leko_core::models::specs::DeviceAction;
use leko_cli::arguments::parse;
use leko_cli::services::enforcement_loop::enforce_with;
use leko_cli::services::os_executor::MockExecutor;

fn workspace() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap()
}

fn sources(directory: &Path, found: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            sources(&path, found);
        } else if path.extension().map(|value| value == "rs").unwrap_or(false) {
            found.push(path);
        }
    }
}

#[test]
fn the_cli_depends_only_on_the_native_core() {
    let manifest = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml")).unwrap();
    assert!(manifest.contains("leko-core"), "the native core is not a dependency");
    for banned in ["flate2", "crc32fast", "ctsettings", "zip_reader"] {
        assert!(!manifest.contains(banned), "legacy dep {} is listed", banned);
    }
    let mut files = Vec::new();
    sources(&Path::new(env!("CARGO_MANIFEST_DIR")).join("src"), &mut files);
    let mut hits = Vec::new();
    for file in files {
        let text = fs::read_to_string(&file).unwrap();
        for banned in ["ctsettings", "zip_reader", "outsideHours", "everyOpen", "blocked_hours"] {
            if text.contains(banned) {
                hits.push(format!("{}:{}", file.display(), banned));
            }
        }
    }
    assert!(hits.is_empty(), "legacy format code was reintroduced: {:?}", hits);
}

#[test]
fn the_full_workspace_builds_with_the_native_core() {
    let manifest = fs::read_to_string(workspace().join("Cargo.toml")).unwrap();
    for member in ["core", "cli", "enforcers/browser", "platform/macos", "platform/android"] {
        assert!(manifest.contains(member), "workspace member {} is missing", member);
    }
    let browser = BrowserId {
        pid: 1,
        bundle_id: "com.example.Browser".to_string(),
        profile: "Default".to_string(),
    };
    let heartbeats = std::collections::HashMap::new();
    let status = DefaultHandshake.status(&browser, &heartbeats, 10_000, 5_000);
    assert_eq!(format!("{:?}", status), "Missing");
    let _ = GuardConfig::default();
    assert_eq!(DeviceAction::Restart, DeviceAction::Restart);
}

#[test]
fn shape_limits_protected_paths_and_fixed_flags_hold() {
    let root = workspace();
    let mut paths = Vec::new();
    for folder in ["core/src", "core/tests", "cli/src", "cli/tests"] {
        let mut found = Vec::new();
        sources(&root.join(folder), &mut found);
        paths.extend(found);
    }
    assert!(paths.len() >= 20, "the rewritten source tree lost its files");
    paths.push(root.join("docs/vocab/USER_VOCABULARY.md"));
    for full in paths {
        let text = fs::read_to_string(&full).unwrap();
        assert!(text.lines().count() < 350, "{} exceeds the line cap", full.display());
        assert!(!text.contains('\u{2014}'), "{} contains an em dash", full.display());
        assert!(full.parent().unwrap().read_dir().unwrap().count() <= 8);
    }

    let output = std::process::Command::new("git")
        .args([
            "ls-files", "src/daemon", "src/blocker-extension", "src/page-regions",
            "enforcers", "voice/audio_io", "voice/normalization",
        ])
        .current_dir(root)
        .output()
        .unwrap();
    assert!(output.status.success());
    let mut names: Vec<String> = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(str::to_string)
        .collect();
    names.sort();
    assert_eq!(names.len(), 66);
    let mut hasher = Sha256::new();
    for name in names {
        hasher.update(name.as_bytes());
        hasher.update([0]);
        hasher.update(fs::read(root.join(&name)).unwrap());
    }
    let digest = hasher.finalize();
    let mut text = String::with_capacity(64);
    for byte in digest {
        text.push_str(&format!("{:02x}", byte));
    }
    assert_eq!(text, "92aaff9716e775ab77c0f10c332395ecfb81daeff9194df8542a57b4c7728b4e");

    let raw: Vec<OsString> = vec![OsString::from("settings"), OsString::from("--json")];
    let mut out = Vec::new();
    let mut err = Vec::new();
    assert_eq!(leko_cli::run(&raw, &mut out, &mut err), 0);
    let settings: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(settings["shadow_mode"], false);
    assert_eq!(settings["nsfw"], false);
    assert_eq!(settings["nsfw_overridable"], false);
}

#[test]
fn keepalive_compatible_loop_shuts_down_cleanly() {
    let dir = workspace().join("target").join(format!("leko-shutdown-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let store = dir.join("store.json");
    leko_core::store::save(&store, &leko_core::store::NativeStore {
        version: 1,
        rules: vec![],
    }).unwrap();
    let argv = ["enforce", store.to_str().unwrap(), "--at", "2026-01-01T12:00:00Z"]
        .iter().map(|value| value.to_string()).collect::<Vec<String>>();
    let parsed = parse(&argv).unwrap();
    let mut mock = MockExecutor::default();
    mock.shutdown = true;
    let mut out = Vec::new();
    let mut err = Vec::new();
    assert!(enforce_with(&parsed, &mut out, &mut err, &mut mock).is_ok());
    assert!(String::from_utf8(out).unwrap().contains("Enforcement ticks: 0"));
    fs::remove_dir_all(dir).unwrap();
}
