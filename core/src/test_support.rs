//! Helpers shared by the tests that scan source files for what may not be mentioned there: a clock,
//! the environment, a time zone database. Hidden from the library's public API.
#![doc(hidden)]

use std::path::{Path, PathBuf};

/// The forbidden words the source scans look for everywhere but in the allowed module: a clock, the
/// environment a zone or a name is taken from, or a time library. The command-line scan adds the
/// process-exit words on top of this list.
pub const CLOCK_SCAN_WORDS: [&str; 14] = [
    "SystemTime",
    "Instant",
    "UNIX_EPOCH",
    "::now(",
    "Utc::",
    "Local::",
    "chrono::",
    "chrono_tz",
    "iana_time_zone",
    "std::env",
    "env::var",
    "libc",
    "std::time",
    "tzset",
];

/// Every `.rs` file under `folder`, found in a stable order.
pub fn rust_files(folder: &Path, found: &mut Vec<PathBuf>) {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(folder).expect("a source folder").map(|entry| entry.expect("an entry").path()).collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            rust_files(&path, found);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            found.push(path);
        }
    }
}

/// The forbidden words in the code lines of `source` (comments are not code).
pub fn forbidden_reads(source: &str, forbidden: &[&'static str]) -> Vec<&'static str> {
    let code = source.lines().filter(|line| !line.trim_start().starts_with("//"));
    let mut found: Vec<&'static str> = code.flat_map(|line| forbidden.iter().copied().filter(move |word| line.contains(word))).collect();
    found.dedup();
    found
}
