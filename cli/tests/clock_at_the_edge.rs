//! Only `main.rs` reads the clock (AC-011): the rest of the command line takes the moment as an
//! argument, so every test can choose it and the same words always make the same session.

use leko_core::test_support::{forbidden_reads, rust_files, CLOCK_SCAN_WORDS};
use std::path::PathBuf;

/// What a source file may not mention outside `main.rs`: a clock, the environment a zone or a name
/// is taken from, a time library, or a process exit.
fn forbidden_words() -> Vec<&'static str> {
    let mut words = CLOCK_SCAN_WORDS.to_vec();
    words.extend(["std::process::exit", "std::process::id"]);
    words
}

fn clock_reads(source: &str) -> Vec<&'static str> {
    forbidden_reads(source, &forbidden_words())
}

fn crate_folder() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn only_main_reads_the_clock() {
    let source = crate_folder().join("src");
    let mut files = Vec::new();
    rust_files(&source, &mut files);
    assert!(files.len() >= 7, "the scan finds the program's files ({})", files.len());

    let mut offenders: Vec<String> = Vec::new();
    let mut readers: Vec<String> = Vec::new();
    for file in &files {
        let relative = file.strip_prefix(&source).expect("under src").to_string_lossy().replace('\\', "/");
        let text = std::fs::read_to_string(file).expect("a source file");
        if text.contains("SystemTime::now()") {
            readers.push(relative.clone());
        }
        if relative == "main.rs" {
            continue;
        }
        let words = clock_reads(&text);
        if !words.is_empty() {
            offenders.push(format!("{relative}: {}", words.join(", ")));
        }
    }
    assert_eq!(offenders, Vec::<String>::new(), "only src/main.rs may read the clock, the environment or the time zone");
    assert_eq!(readers, vec!["main.rs".to_string()], "main.rs is the one place the clock is read");

    // main.rs hands the moment to the library and does nothing else with it.
    let main = std::fs::read_to_string(source.join("main.rs")).expect("main.rs");
    assert!(main.lines().count() <= 40, "main.rs stays a thin start-up file ({} lines)", main.lines().count());
    assert!(main.contains("leko_cli::run("), "main.rs calls the library's run");

    // The library's entry point takes the moment as an argument; nothing reads it inside.
    let run_command = std::fs::read_to_string(source.join("session/run_command.rs")).expect("run_command.rs");
    assert!(run_command.contains("pub fn run(args: &[OsString], now_ms: i64,"), "run takes the moment as an argument");

    // No clock or time-zone library is a dependency of the program.
    let manifest = std::fs::read_to_string(crate_folder().join("Cargo.toml")).expect("Cargo.toml");
    for library in ["chrono", "time", "iana-time-zone", "libc", "tz"] {
        assert!(!manifest.lines().any(|line| line.trim_start().starts_with(&format!("{library} ")) || line.trim_start().starts_with(&format!("{library}="))), "{library} is not a dependency");
    }
}

#[test]
fn the_scan_catches_a_clock_read() {
    assert_eq!(clock_reads("let now = std::time::SystemTime::now();"), vec!["SystemTime", "::now(", "std::time"]);
    assert_eq!(clock_reads("let zone = std::env::var(\"TZ\");"), vec!["std::env", "env::var"]);
    assert_eq!(clock_reads("let at = chrono::Utc::now();"), vec!["::now(", "Utc::", "chrono::"]);
    assert_eq!(clock_reads("std::process::exit(2);"), vec!["std::process::exit"]);
    assert!(clock_reads("// it never calls SystemTime::now()\nlet x = 1;").is_empty(), "a comment is not a read");
}
