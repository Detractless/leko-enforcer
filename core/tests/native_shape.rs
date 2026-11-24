use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn source_files(directory: &Path, found: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            source_files(&path, found);
        } else if path.extension().map(|value| value == "rs").unwrap_or(false) {
            found.push(path);
        }
    }
}

#[test]
fn only_the_clock_adapter_reads_the_time() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    source_files(&root, &mut files);
    let mut hits = Vec::new();
    for path in files {
        let text = fs::read_to_string(&path).unwrap();
        for term in ["Utc::now()", "SystemTime::now()", "Local::now()", "now_utc()"] {
            if text.contains(term) {
                hits.push(path.clone());
            }
        }
    }
    hits.sort();
    hits.dedup();
    assert_eq!(hits, vec![root.join("io/clock.rs")], "a clock read escaped the adapter");
}

#[test]
fn new_files_stay_under_350_lines_and_directories_under_8_entries() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let plan = fs::read_to_string(workspace.join(".agent/plans/ACTIVE.md")).unwrap();
    let mut paths: Vec<String> = plan
        .lines()
        .filter_map(|line| line.strip_prefix("### ").map(str::trim))
        .map(str::to_string)
        .collect();
    assert!(paths.len() >= 20, "the active plan lost its M1 entries");
    paths.push("docs/vocab/USER_VOCABULARY.md".to_string());

    for relative in paths {
        let full = workspace.join(&relative);
        assert!(full.exists(), "planned file {} is missing", relative);
        let text = fs::read_to_string(&full).unwrap();
        assert!(text.lines().count() < 350, "{} exceeds the line cap", relative);
        assert!(!text.contains('\u{2014}'), "{} contains an em dash", relative);
        let parent = full.parent().unwrap();
        let entries = fs::read_dir(parent).unwrap().count();
        assert!(entries <= 8, "{} has {} immediate entries", parent.display(), entries);
    }
}

#[test]
fn protected_paths_are_byte_identical_to_main() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let output = Command::new("git")
        .args([
            "ls-files",
            "src/daemon",
            "src/blocker-extension",
            "src/page-regions",
            "enforcers",
            "voice/audio_io",
            "voice/normalization",
        ])
        .current_dir(workspace)
        .output()
        .unwrap();
    assert!(output.status.success(), "git ls-files failed");
    let mut names: Vec<String> = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(str::to_string)
        .collect();
    names.sort();
    assert_eq!(names.len(), 66, "the protected file list changed");

    let mut hasher = Sha256::new();
    for name in names {
        hasher.update(name.as_bytes());
        hasher.update([0]);
        hasher.update(fs::read(workspace.join(&name)).unwrap());
    }
    let digest = hasher.finalize();
    let mut text = String::with_capacity(64);
    for byte in digest {
        text.push_str(&format!("{:02x}", byte));
    }
    assert_eq!(
        text,
        "92aaff9716e775ab77c0f10c332395ecfb81daeff9194df8542a57b4c7728b4e",
        "a protected path was modified"
    );
}
