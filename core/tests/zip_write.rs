//! The core writes a zip that its own reader, the phone's way of reading, and an ordinary unzip tool
//! all read back the same, and it refuses the lists its own reader would refuse, with the same
//! sentences (AC-012). The zips here are stored layouts with sizes in the local headers.

mod zip_writer;

use leko_core::io::zip_reader::open_zip;
use leko_core::io::zip_writer::write_zip;
use std::path::PathBuf;
use std::process::Command;
use zip_writer::{build_zip, Entry, Method};

fn file(name: &str, data: &[u8]) -> (String, Vec<u8>) {
    (name.to_string(), data.to_vec())
}

fn sorted(mut files: Vec<(String, Vec<u8>)>) -> Vec<(String, Vec<u8>)> {
    files.sort_by(|left, right| left.0.cmp(&right.0));
    files
}

/// What the reader says about the same list written by the test's own writer: the sentence it
/// refuses with, or nothing when it reads the zip.
fn reader_sentence(files: &[(String, Vec<u8>)]) -> Option<String> {
    let entries: Vec<Entry> = files.iter().map(|(name, data)| Entry { name: name.as_bytes().to_vec(), data: data.clone() }).collect();
    open_zip(&build_zip(&entries, Method::Stored)).err()
}

fn scratch_zip(tag: &str, bytes: &[u8]) -> PathBuf {
    let path = std::env::temp_dir().join(format!("leko-zip-write-{}-{tag}.zip", std::process::id()));
    std::fs::write(&path, bytes).expect("a scratch zip is written");
    path
}

#[test]
fn written_zips_read_back_the_same_in_the_core_and_in_unzip() {
    let mut long = Vec::new();
    for count in 0..60_000u32 {
        long.extend_from_slice(&count.to_le_bytes());
    }
    let lists: Vec<Vec<(String, Vec<u8>)>> = vec![
        vec![file("GUIDE.md", b"guide")],
        vec![file("blocks/b.json", b"{\"a\":1}\n"), file("GUIDE.md", b"guide"), file("empty.txt", b""), file("general.json", br#"{"schema": 2}"#)],
        vec![file("blocks/caf\u{e9} \u{65e5}\u{672c}.json", "na\u{ef}ve \u{1f600}".as_bytes()), file("zero.bin", &[0u8; 1000]), file("long.bin", &long)],
        (0..500).map(|index| file(&format!("blocks/b{index:03}.json"), format!("{{\"n\":{index}}}").as_bytes())).collect(),
    ];
    let unzip_works = Command::new("unzip").arg("-v").output().is_ok();
    for (index, files) in lists.iter().enumerate() {
        let bytes = write_zip(files).unwrap_or_else(|sentence| panic!("list {index}: {sentence}"));
        assert_eq!(write_zip(files), Ok(bytes.clone()), "list {index}: the same list gives the same bytes");

        let opened = open_zip(&bytes).unwrap_or_else(|sentence| panic!("list {index}: the core's reader refused: {sentence}"));
        assert_eq!(opened.files, sorted(files.clone()), "list {index}: names and contents read back");
        assert!(opened.duplicates.is_empty(), "list {index}");

        if unzip_works {
            let path = scratch_zip(&index.to_string(), &bytes);
            let test = Command::new("unzip").arg("-tq").arg(&path).output().expect("unzip runs");
            assert!(test.status.success(), "list {index}: unzip -t said {}", String::from_utf8_lossy(&test.stdout));
            for (name, data) in files.iter().filter(|(name, _)| !name.contains(['\u{e9}', '\u{65e5}'])) {
                let shown = Command::new("unzip").arg("-p").arg(&path).arg(name).output().expect("unzip runs");
                assert_eq!(&shown.stdout, data, "list {index}: unzip shows {name} as it was written");
            }
            let _ = std::fs::remove_file(&path);
        }
    }
}

#[test]
fn a_list_the_reader_would_refuse_is_refused_with_the_readers_sentence() {
    let big = vec![7u8; 1024 * 1024 + 1];
    let same_name = vec![file("a.txt", b"one"), file("a.txt", b"two")];
    let too_many: Vec<(String, Vec<u8>)> = (0..501).map(|index| file(&format!("f{index}"), b"x")).collect();
    let mega = vec![5u8; 1024 * 1024];
    let too_much: Vec<(String, Vec<u8>)> = (0..65).map(|index| file(&format!("m{index}"), &mega)).collect();

    let big_one = vec![file("big.bin", &big)];
    assert_eq!(write_zip(&big_one).err(), reader_sentence(&big_one), "an entry over 1 MB");
    assert_eq!(write_zip(&too_many).err(), reader_sentence(&too_many), "more than 500 files");
    assert_eq!(write_zip(&too_much).err(), reader_sentence(&too_much), "more than 64 MB in all");
    assert!(write_zip(&big_one).is_err() && write_zip(&too_many).is_err() && write_zip(&too_much).is_err());

    assert_eq!(write_zip(&[]).err(), reader_sentence(&[]), "no file at all");

    let twice = write_zip(&same_name).expect_err("a name used twice is refused");
    assert!(twice.contains("a.txt") && twice.contains("twice"), "{twice}");

    for bad in ["", "folder/", &"n".repeat(1025)] {
        let sentence = write_zip(&[file(bad, b"x")]).expect_err("an unwritable name is refused");
        assert!(sentence.contains("cannot be written"), "{sentence}");
    }
    assert!(write_zip(&[file("exactly.txt", &vec![1u8; 1024 * 1024])]).is_ok(), "an entry of exactly 1 MB is written");
    assert!(write_zip(&(0..500).map(|index| file(&format!("f{index}"), b"x")).collect::<Vec<_>>()).is_ok(), "500 files are written");
}
