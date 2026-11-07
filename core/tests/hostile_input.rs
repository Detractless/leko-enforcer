//! Empty, truncated, non-UTF-8 and very deeply nested input never crashes the library; each gives a
//! refusal, never a panic.

mod zip_writer;

use leko_core::io::zip_check::{check_zip, ZipVerdict};
use std::collections::HashSet;
use std::panic::{catch_unwind, AssertUnwindSafe};
use zip_writer::{build_zip, zip_of, Entry, Method};

const GOOD_BLOCK: &str = r#"{"schema":2,"id":"a","name":"A","covers":{"apps":["com.a"]},"rules":[{"id":"m","kind":"minutes","amount":30,"per":"day","scope":"block"}]}"#;
const DEPTH: usize = 200_000;

/// What the core says of `bytes`: `Ok(true)` refused, `Ok(false)` read, `Err` when it panicked.
fn verdict(bytes: &[u8]) -> Result<bool, ()> {
    catch_unwind(AssertUnwindSafe(|| matches!(check_zip(bytes, &HashSet::new()), ZipVerdict::Refused(_)))).map_err(|_| ())
}

fn zip_with_block(block: &[u8]) -> Vec<u8> {
    let entries = [Entry { name: b"GUIDE.md".to_vec(), data: b"guide".to_vec() }, Entry { name: b"blocks/a.json".to_vec(), data: block.to_vec() }];
    build_zip(&entries, Method::Deflate)
}

fn nested(open: &str, close: &str) -> String {
    format!("{}{}", open.repeat(DEPTH), close.repeat(DEPTH))
}

#[test]
fn hostile_input_is_refused_not_a_panic() {
    let mut cases: Vec<(String, Vec<u8>)> = Vec::new();
    let mut add = |name: &str, bytes: Vec<u8>| cases.push((name.to_string(), bytes));

    // Empty and tiny.
    add("empty", Vec::new());
    add("one byte", vec![0x50]);
    add("a local header signature alone", b"PK\x03\x04".to_vec());
    add("an end record alone", b"PK\x05\x06".to_vec());
    add("plain text", b"this is not a zip".to_vec());
    add("an end record that claims 65535 files far away", {
        let mut end = vec![0u8; 22];
        end[..4].copy_from_slice(b"PK\x05\x06");
        end[8..12].copy_from_slice(&[0xff; 4]);
        end[12..16].copy_from_slice(&[0xff; 4]);
        end[16..20].copy_from_slice(&[0xff; 4]);
        end
    });

    let whole = zip_of(&[("GUIDE.md", "guide"), ("blocks/a.json", GOOD_BLOCK)]);
    // A truncated file inside a good zip.
    add("a block cut off in the middle", zip_with_block(&GOOD_BLOCK.as_bytes()[..GOOD_BLOCK.len() / 2]));
    add("a block cut off after a colon", zip_with_block(br#"{"schema":2,"id":"#));

    // Not UTF-8.
    add("a block of bytes that are not UTF-8", zip_with_block(&[0xff, 0xfe, 0x00, 0x80, 0x81]));
    add("a block with a stray byte among the keys", zip_with_block(b"{\"schema\":2,\xff\"id\":\"a\"}"));
    add("a block that is only a byte order mark", zip_with_block(&[0xef, 0xbb, 0xbf]));
    add("a zip whose entry name is not UTF-8", build_zip(&[Entry { name: vec![0xff, 0xfe, b'.', b'j'], data: b"{}".to_vec() }], Method::Deflate));

    // Very deeply nested.
    add("arrays nested deeply as the whole file", zip_with_block(nested("[", "]").as_bytes()));
    add("objects nested deeply as the whole file", zip_with_block(format!("{}1{}", "{\"a\":".repeat(DEPTH), "}".repeat(DEPTH)).as_bytes()));
    add("arrays nested deeply inside a field", zip_with_block(format!("{{\"schema\":2,\"id\":\"a\",\"name\":\"A\",\"covers\":{{\"apps\":{}}}}}", nested("[", "]")).as_bytes()));
    add("an unclosed run of brackets", zip_with_block("[".repeat(DEPTH).as_bytes()));

    let panicked: Vec<&str> = cases.iter().filter(|(_, bytes)| verdict(bytes).is_err()).map(|(name, _)| name.as_str()).collect();
    assert_eq!(panicked, Vec::<&str>::new(), "these inputs made the library panic");
    let read: Vec<&str> = cases.iter().filter(|(_, bytes)| verdict(bytes) == Ok(false)).map(|(name, _)| name.as_str()).collect();
    assert_eq!(read, Vec::<&str>::new(), "these inputs were read, not refused");
    assert_eq!(verdict(&whole), Ok(false), "the whole zip is read");
}

/// The phone reads a zip entry by entry, so a zip cut inside its list of entries (after every file
/// is whole) still holds all its files. Any other cut loses part of a file and is refused. A cut
/// zip is never read in part.
#[test]
fn a_truncated_zip_is_refused_or_read_whole_never_in_part() {
    let whole = zip_of(&[("GUIDE.md", "guide"), ("blocks/a.json", GOOD_BLOCK)]);
    let listing = |bytes: &[u8]| match check_zip(bytes, &HashSet::new()) {
        ZipVerdict::Read(read) => Some(read.listing),
        ZipVerdict::Refused(_) => None,
    };
    let full = listing(&whole).expect("the whole zip is read");
    let list_starts = whole.windows(4).position(|window| window == b"PK\x01\x02").expect("a central list");
    for length in 0..whole.len() {
        let got = catch_unwind(AssertUnwindSafe(|| listing(&whole[..length]))).unwrap_or_else(|_| panic!("a zip cut at {length} made the library panic"));
        if length < list_starts {
            assert_eq!(got, None, "a zip cut at {length} of {} bytes, inside a file, must be refused", whole.len());
        } else if let Some(read) = got {
            assert_eq!(read, full, "a zip cut at {length} was read in part");
        }
    }
}
