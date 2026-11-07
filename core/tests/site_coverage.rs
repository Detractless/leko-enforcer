//! A block can cover sites. `covers.sites` holds 1 to 1000 lowercase domain names, alone or beside
//! `apps` or `allAppsExcept`; any other site text refuses the whole zip and names the file and field.

mod zip_writer;

use leko_core::io::zip_check::{check_zip, Refusal, ZipVerdict};
use serde_json::{json, Value};
use std::collections::HashSet;
use zip_writer::zip_of;

const RULES: &str = r#"[{"id":"m","kind":"minutes","amount":30,"per":"day","scope":"block"}]"#;

fn block_file(id: &str, covers: &Value) -> String {
    format!(r#"{{"schema":2,"id":"{id}","name":"B","covers":{covers},"rules":{RULES}}}"#)
}

fn verdict_of(covers: &Value) -> ZipVerdict {
    let good = block_file("a", &json!({"apps": ["com.a"]}));
    let under_test = block_file("b", covers);
    check_zip(&zip_of(&[("GUIDE.md", "guide"), ("blocks/a.json", &good), ("blocks/b.json", &under_test)]), &HashSet::new())
}

fn refusal_of(covers: &Value) -> Refusal {
    match verdict_of(covers) {
        ZipVerdict::Refused(refusal) => refusal,
        ZipVerdict::Read(_) => panic!("expected a refusal for {covers}, but the zip was read"),
    }
}

fn sites_read(covers: &Value) -> Vec<String> {
    match verdict_of(covers) {
        ZipVerdict::Read(read) => read.set.blocks.get("b").unwrap_or_else(|| panic!("block b is missing")).sites.clone(),
        ZipVerdict::Refused(refusal) => panic!("expected {covers} to be read, but it was refused: {}", refusal.sentence),
    }
}

fn names(count: usize) -> Vec<String> {
    (0..count).map(|number| format!("site{number}.example.com")).collect()
}

/// A name of exactly `length` characters made of labels of at most `label` characters.
fn name_of_length(length: usize, label: usize) -> String {
    let mut parts: Vec<String> = Vec::new();
    let mut left = length;
    while left > 0 {
        let take = if left > label + 1 { label } else { left };
        parts.push("a".repeat(take));
        left -= take + usize::from(left > take);
    }
    parts.join(".")
}

#[test]
fn sites_are_read_and_bad_sites_refuse_the_whole_zip() {
    // Good: alone, beside either app list, a Mac bundle id in apps, the limits.
    assert_eq!(sites_read(&json!({"sites": ["instagram.com"]})), vec!["instagram.com"]);
    assert_eq!(sites_read(&json!({"sites": ["a.co", "m.facebook.com", "xn--bcher-kva.example", "my-site.co.uk", "x.c0m", "1.example"]})).len(), 6);
    assert_eq!(sites_read(&json!({"apps": ["com.apple.Safari"], "sites": ["reddit.com"]})), vec!["reddit.com"]);
    assert_eq!(sites_read(&json!({"allAppsExcept": [], "sites": ["reddit.com"]})), vec!["reddit.com"]);
    assert_eq!(sites_read(&json!({"apps": ["com.a"]})), Vec::<String>::new());
    assert_eq!(sites_read(&json!({"sites": names(1000)})).len(), 1000);
    let longest = name_of_length(253, 63);
    assert_eq!(longest.len(), 253);
    assert_eq!(sites_read(&json!({"sites": [longest]})).len(), 1);
    assert_eq!(sites_read(&json!({"sites": [format!("{}.com", "a".repeat(63))]})).len(), 1);

    // Bad: each is refused whole, naming blocks/b.json and the sites field, and nothing is read.
    let too_long = name_of_length(254, 63);
    let long_label = format!("{}.com", "a".repeat(64));
    let nested = json!([[["a.com"]]]);
    let bad: Vec<(&str, Value)> = vec![
        ("a URL", json!({"sites": ["https://instagram.com"]})),
        ("a path", json!({"sites": ["instagram.com/feed"]})),
        ("a port", json!({"sites": ["instagram.com:443"]})),
        ("a wildcard", json!({"sites": ["*.instagram.com"]})),
        ("upper case", json!({"sites": ["Instagram.com"]})),
        ("an IP number", json!({"sites": ["192.168.0.1"]})),
        ("a single label", json!({"sites": ["localhost"]})),
        ("a space", json!({"sites": ["insta gram.com"]})),
        ("an underscore", json!({"sites": ["insta_gram.com"]})),
        ("a non-ASCII letter", json!({"sites": ["münchen.de"]})),
        ("a leading dash", json!({"sites": ["-a.com"]})),
        ("a trailing dash", json!({"sites": ["a-.com"]})),
        ("an empty label", json!({"sites": ["a..com"]})),
        ("a leading dot", json!({"sites": [".a.com"]})),
        ("a trailing dot", json!({"sites": ["a.com."]})),
        ("a number as last label", json!({"sites": ["example.123"]})),
        ("an empty name", json!({"sites": [""]})),
        ("a line break", json!({"sites": ["a.com\n"]})),
        ("a name over 253 characters", json!({"sites": [too_long]})),
        ("a label over 63 characters", json!({"sites": [long_label]})),
        ("a number", json!({"sites": [5]})),
        ("a null", json!({"sites": [null]})),
        ("a list inside the list", json!({"sites": [["a.com"]]})),
        ("lists nested deeper", json!({"sites": nested})),
        ("one bad name among good ones", json!({"sites": ["a.com", "b.com", "C.com"]})),
        ("an empty list", json!({"sites": []})),
        ("an empty list beside apps", json!({"apps": ["com.a"], "sites": []})),
        ("the 1001st site", json!({"sites": names(1001)})),
        ("a list of 100000 names that fits the file limit", json!({"sites": vec!["a.co"; 100_000]})),
        ("a 900 KB string that fits the file limit", json!({"sites": ["a".repeat(900_000)]})),
        ("text instead of a list", json!({"sites": "instagram.com"})),
        ("null instead of a list", json!({"sites": null})),
        ("an object instead of a list", json!({"sites": {"a.com": 1}})),
    ];
    for (label, covers) in &bad {
        let refusal = refusal_of(covers);
        assert_eq!(refusal.entry.as_deref(), Some("blocks/b.json"), "{label}: the refusal names the file");
        assert!(refusal.sentence.starts_with("blocks/b.json: "), "{label}: {}", refusal.sentence);
        assert!(refusal.sentence.contains("sites"), "{label}: the sentence names the field: {}", refusal.sentence);
        let marked: Vec<&str> = refusal.files.iter().filter(|file| file.problem.is_some()).map(|file| file.path.as_str()).collect();
        assert_eq!(marked, vec!["blocks/b.json"], "{label}: only the bad file carries the problem");
    }

    // Past the 1 MB file limit the zip reader refuses first; it is still a refusal, never a crash.
    for covers in [json!({"sites": names(100_000)}), json!({"sites": ["a".repeat(10_000_000)]})] {
        assert!(matches!(verdict_of(&covers), ZipVerdict::Refused(_)));
    }

    // The good block next to a bad one is not accepted on its own.
    let named = refusal_of(&json!({"sites": ["a.com", "C.com"]}));
    assert!(named.sentence.contains("sites[1]"), "the second name is the bad one: {}", named.sentence);

    // Coverage is still one of the three forms: both lists, or none, is refused as before.
    for covers in [json!({"apps": ["com.a"], "allAppsExcept": [], "sites": ["a.com"]}), json!({})] {
        let refusal = refusal_of(&covers);
        assert!(refusal.sentence.contains("must hold either \"apps\" or \"allAppsExcept\""), "{covers}: {}", refusal.sentence);
    }
}
