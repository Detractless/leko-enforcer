//! Builds the settings files a status test asks about: block files written as JSON and put in a zip
//! the way the phone exports it (deflated entries with a data descriptor), then written to a folder
//! the test owns. The zip writer is the core tests' own, cut down to the one layout the phone uses.
#![allow(dead_code)]

use flate2::write::DeflateEncoder;
use flate2::Compression;
use serde_json::Value;
#[allow(unused_imports)]
pub use leko_core::test_support::{always, block, block_with, minutes, opens, outside_hours, session};

use std::io::Write;
use std::path::{Path, PathBuf};

fn put16(bytes: &mut Vec<u8>, value: u16) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn put32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn deflate(data: &[u8]) -> Vec<u8> {
    let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(data).expect("deflate into memory");
    encoder.finish().expect("deflate into memory")
}

/// The bytes of a zip holding the named text files, deflated the way the phone writes them.
pub fn zip_of(files: &[(&str, &str)]) -> Vec<u8> {
    const UTF8_AND_DESCRIPTOR: u16 = 0x0808;
    let (mut out, mut central) = (Vec::new(), Vec::new());
    for (name, text) in files {
        let (packed, crc, offset) = (deflate(text.as_bytes()), crc32fast::hash(text.as_bytes()), out.len() as u32);
        out.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
        for field in [20, UTF8_AND_DESCRIPTOR, 8, 0, 0x21] {
            put16(&mut out, field);
        }
        for field in [0, 0, 0] {
            put32(&mut out, field);
        }
        put16(&mut out, name.len() as u16);
        put16(&mut out, 0);
        out.extend_from_slice(name.as_bytes());
        out.extend_from_slice(&packed);
        for field in [0x0807_4b50, crc, packed.len() as u32, text.len() as u32] {
            put32(&mut out, field);
        }
        put32(&mut central, 0x0201_4b50);
        for field in [20, 20, UTF8_AND_DESCRIPTOR, 8, 0, 0x21] {
            put16(&mut central, field);
        }
        for field in [crc, packed.len() as u32, text.len() as u32] {
            put32(&mut central, field);
        }
        for field in [name.len() as u16, 0, 0, 0, 0] {
            put16(&mut central, field);
        }
        put32(&mut central, 0);
        put32(&mut central, offset);
        central.extend_from_slice(name.as_bytes());
    }
    let (start, size) = (out.len() as u32, central.len() as u32);
    out.extend(central);
    put32(&mut out, 0x0605_4b50);
    for field in [0, 0, files.len() as u16, files.len() as u16] {
        put16(&mut out, field);
    }
    put32(&mut out, size);
    put32(&mut out, start);
    put16(&mut out, 0);
    out
}

/// The bytes of a settings zip holding `blocks`, written in this order, after the guide and the
/// general file every export has (a zip with no settings file at all is refused by the core).
pub fn settings_bytes(blocks: &[Value]) -> Vec<u8> {
    let files: Vec<(String, String)> = blocks.iter().map(|file| (format!("blocks/{}.json", file["id"].as_str().unwrap_or("x")), file.to_string())).collect();
    let mut entries: Vec<(&str, &str)> = vec![("GUIDE.md", "guide"), ("general.json", r#"{"schema": 2}"#)];
    entries.extend(files.iter().map(|(path, text)| (path.as_str(), text.as_str())));
    zip_of(&entries)
}

/// A settings zip holding `blocks`, written as `name` into `folder`.
pub fn write_settings(folder: &Path, name: &str, blocks: &[Value]) -> PathBuf {
    let path = folder.join(name);
    std::fs::write(&path, settings_bytes(blocks)).expect("a settings file is written");
    path
}
