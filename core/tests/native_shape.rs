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
    let mut paths = Vec::new();
    for folder in ["core/src", "core/tests", "cli/src", "cli/tests"] {
        let mut found = Vec::new();
        source_files(&workspace.join(folder), &mut found);
        paths.extend(found);
    }
    assert!(paths.len() >= 20, "the rewritten source tree lost its files");
    paths.push(workspace.join("docs/vocab/USER_VOCABULARY.md"));

    for full in paths {
        let text = fs::read_to_string(&full).unwrap();
        assert!(text.lines().count() < 350, "{} exceeds the line cap", full.display());
        assert!(!text.contains('\u{2014}'), "{} contains an em dash", full.display());
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
