//! A refused zip is refused whole, and the message names the file and the field in plain words. A
//! file stamped with a schema number other than 2 is refused with a message that says only format
//! 2 is read.

mod zip_writer;

use leko_core::io::zip_check::{check_zip, Refusal, ZipVerdict};
use std::collections::HashSet;
use zip_writer::zip_of;

const GOOD_BLOCK: &str = r#"{"schema":2,"id":"a","name":"A","covers":{"apps":["com.a"]},"rules":[{"id":"m","kind":"minutes","amount":30,"per":"day","scope":"block"}]}"#;
const BAD_BLOCK: &str = r#"{"schema":2,"id":"b","name":"B","covers":{"apps":["com.b"]},"rules":[{"id":"m","kind":"minutes","amount":1000001,"per":"day","scope":"block"}]}"#;

fn refusal_of(files: &[(&str, &str)]) -> Refusal {
    match check_zip(&zip_of(files), &HashSet::new()) {
        ZipVerdict::Refused(refusal) => refusal,
        ZipVerdict::Read(read) => panic!("expected a refusal, but the zip was read: {:?}", read.listing),
    }
}

#[test]
fn refusal_is_whole_and_names_file_and_field() {
    // The zip with only the good file reads.
    let alone = [("GUIDE.md", "guide"), ("blocks/a.json", GOOD_BLOCK)];
    assert!(matches!(check_zip(&zip_of(&alone), &HashSet::new()), ZipVerdict::Read(_)), "the good file alone is read");

    // One good file and one bad file, in either order: nothing is accepted, and the bad file is named.
    for files in [
        [("GUIDE.md", "guide"), ("blocks/a.json", GOOD_BLOCK), ("blocks/b.json", BAD_BLOCK)],
        [("GUIDE.md", "guide"), ("blocks/b.json", BAD_BLOCK), ("blocks/a.json", GOOD_BLOCK)],
    ] {
        let refusal = refusal_of(&files);
        assert_eq!(refusal.entry.as_deref(), Some("blocks/b.json"), "the refusal names the file");
        assert!(refusal.sentence.starts_with("blocks/b.json: "), "the sentence starts with the file: {}", refusal.sentence);
        assert!(refusal.sentence.contains("\"rules[0].amount\""), "the sentence names the field: {}", refusal.sentence);
        assert!(refusal.sentence.contains("must be a whole number from 0 to 1000000"), "the sentence says what is wrong: {}", refusal.sentence);
        assert!(!refusal.sentence.to_lowercase().contains("invalid"), "the sentence is not a generic \"invalid\": {}", refusal.sentence);
        // Only the bad file carries a problem; the good one is not what was refused, yet it is not accepted either.
        let marked: Vec<&str> = refusal.files.iter().filter(|file| file.problem.is_some()).map(|file| file.path.as_str()).collect();
        assert_eq!(marked, vec!["blocks/b.json"]);
        assert_eq!(refusal.problems.len(), 1);
    }
}

#[test]
fn a_schema_number_other_than_2_says_only_format_2_is_read() {
    for stamp in [r#""schema":1"#, r#""schema":3"#, r#""schema":"2""#, r#""schema":2.5"#, r#""schema":null"#, r#""other":1"#] {
        let general = format!("{{{stamp}}}");
        let refusal = refusal_of(&[("GUIDE.md", "guide"), ("general.json", &general)]);
        assert_eq!(refusal.entry.as_deref(), Some("general.json"), "{stamp}");
        assert!(refusal.sentence.contains("\"schema\""), "{stamp}: names the field: {}", refusal.sentence);
        assert!(refusal.sentence.contains("Only format 2 is read"), "{stamp}: {}", refusal.sentence);
    }
    // The same in a block, next to a good file.
    let block = GOOD_BLOCK.replace("\"schema\":2", "\"schema\":3").replace("\"id\":\"a\"", "\"id\":\"c\"");
    let refusal = refusal_of(&[("GUIDE.md", "guide"), ("blocks/a.json", GOOD_BLOCK), ("blocks/c.json", &block)]);
    assert_eq!(refusal.entry.as_deref(), Some("blocks/c.json"));
    assert!(refusal.sentence.contains("Only format 2 is read"), "{}", refusal.sentence);
}
