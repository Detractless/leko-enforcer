//! Only the time module reads the clock or the time zone; the judge takes the time and zone as input.

use leko_core::test_support::{forbidden_reads, rust_files, CLOCK_SCAN_WORDS};
use std::path::Path;

fn clock_reads(source: &str) -> Vec<&'static str> {
    forbidden_reads(source, &CLOCK_SCAN_WORDS)
}

#[test]
fn only_the_time_module_reads_the_clock() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    rust_files(&source, &mut files);
    assert!(files.len() >= 25, "the scan finds the library's files ({})", files.len());

    let mut offenders: Vec<String> = Vec::new();
    for file in &files {
        let relative = file.strip_prefix(&source).expect("under src").to_string_lossy().replace('\\', "/");
        if relative.starts_with("config_time/") || relative == "test_support.rs" {
            continue;
        }
        let text = std::fs::read_to_string(file).expect("a source file");
        let words = clock_reads(&text);
        if !words.is_empty() {
            offenders.push(format!("{relative}: {}", words.join(", ")));
        }
    }
    assert_eq!(offenders, Vec::<String>::new(), "only src/config_time may read the clock or the time zone");

    // Not even the time module reads the clock: the moment of a request and the zone are arguments.
    let due_time = std::fs::read_to_string(source.join("config_time/due_time.rs")).expect("due_time.rs");
    for word in ["SystemTime", "Instant", "::now(", "UNIX_EPOCH"] {
        assert!(!clock_reads(&due_time).contains(&word), "due_time.rs reads the clock with {word}");
    }
    assert!(due_time.contains("pub fn due(requested_g: i64, locks: &[Lock], zone: &ZoneTime)"), "the landing time takes the moment and the zone");
    let item_diff = std::fs::read_to_string(source.join("config_diff/item_diff.rs")).expect("item_diff.rs");
    assert!(item_diff.contains("pub requested_g: i64") && item_diff.contains("pub zone: &'a ZoneTime"), "the judge takes the moment and the zone");
}

#[test]
fn the_scan_catches_a_clock_read() {
    assert_eq!(clock_reads("let now = std::time::SystemTime::now();"), vec!["SystemTime", "::now(", "std::time"]);
    assert_eq!(clock_reads("let zone = std::env::var(\"TZ\");"), vec!["std::env", "env::var"]);
    assert_eq!(clock_reads("let at = chrono::Utc::now();"), vec!["::now(", "Utc::", "chrono::"]);
    assert!(clock_reads("// it never calls SystemTime::now()\nlet x = 1;").is_empty(), "a comment is not a read");
}
