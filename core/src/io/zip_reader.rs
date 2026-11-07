//! Opens a zip the way the phone does (java.util.zip.ZipInputStream): local headers in order from
//! byte 0, a data descriptor after a deflated entry whose header holds no sizes, and a stop at the
//! first thing that is not a local header, which is the central directory on a healthy zip. Bytes
//! in front of the zip, an entry only the central directory lists and a zip cut short therefore
//! read as the phone reads them. The bounds are the phone's, and an entry is inflated only up to
//! the bytes the zip may still hold, so a deflate bomb stops at the bound.
//!
//! Ported from Enforcer's `tools/enforcer-setup/lib/zip-reader.js`.

use flate2::{Decompress, FlushDecompress, Status};
use std::collections::BTreeMap;

pub const MAX_ENTRY_BYTES: usize = 1024 * 1024;
pub const MAX_ENTRIES: usize = 500;
pub const MAX_TOTAL_BYTES: usize = 64 * 1024 * 1024;

const LOCAL_SIGNATURE: u32 = 0x0403_4b50;
const DESCRIPTOR_SIGNATURE: u32 = 0x0807_4b50;
const CHUNK_BYTES: usize = 16 * 1024;

pub const NOT_A_ZIP: &str = "This file could not be opened as a zip.";
const NOT_UTF8: &str = "An entry name in this zip is not written in UTF-8.";
pub const TOO_MUCH: &str = "This zip holds more data than a settings file should.";

/// The files of a zip that opened, in code point order of their names, and the names that
/// appeared more than once (only the first copy of each is kept).
#[derive(Debug)]
pub struct OpenedZip {
    pub files: Vec<(String, Vec<u8>)>,
    pub duplicates: Vec<String>,
}

struct Header {
    name: String,
    flag: u16,
    method: u16,
    crc: u32,
    packed: u32,
    size: u32,
    data_start: usize,
}

struct Descriptor {
    crc: u32,
    packed: u32,
    size: u32,
    length: usize,
}

/// The sentence for a zip with more files than the phone reads; the writer refuses with the same one.
pub fn too_many_files() -> String {
    format!("This zip holds more than {MAX_ENTRIES} files.")
}

/// The sentence for a file larger than the phone reads; the writer refuses with the same one.
pub fn entry_too_big(name: &str) -> String {
    format!("The entry \"{name}\" is larger than {} MB.", MAX_ENTRY_BYTES / (1024 * 1024))
}

fn refuse(sentence: &str) -> String {
    sentence.to_string()
}

fn need(bytes: &[u8], end: usize) -> Result<(), String> {
    if end > bytes.len() {
        return Err(refuse(NOT_A_ZIP));
    }
    Ok(())
}

fn u16_at(bytes: &[u8], at: usize) -> Result<u16, String> {
    match bytes.get(at..at + 2) {
        Some(b) => Ok(u16::from_le_bytes([b[0], b[1]])),
        None => Err(refuse(NOT_A_ZIP)),
    }
}

fn u32_at(bytes: &[u8], at: usize) -> Result<u32, String> {
    match bytes.get(at..at + 4) {
        Some(b) => Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]])),
        None => Err(refuse(NOT_A_ZIP)),
    }
}

fn read_header(bytes: &[u8], at: usize) -> Result<Option<Header>, String> {
    if at + 30 > bytes.len() || u32_at(bytes, at)? != LOCAL_SIGNATURE {
        return Ok(None);
    }
    let flag = u16_at(bytes, at + 6)?;
    let method = u16_at(bytes, at + 8)?;
    let name_end = at + 30 + u16_at(bytes, at + 26)? as usize;
    let data_start = name_end + u16_at(bytes, at + 28)? as usize;
    need(bytes, data_start)?;
    let name = String::from_utf8(bytes[at + 30..name_end].to_vec()).map_err(|_| refuse(NOT_UTF8))?;
    if flag & 1 != 0 || (flag & 8 != 0 && method != 8) {
        return Err(refuse(NOT_A_ZIP));
    }
    Ok(Some(Header {
        name,
        flag,
        method,
        crc: u32_at(bytes, at + 14)?,
        packed: u32_at(bytes, at + 18)?,
        size: u32_at(bytes, at + 22)?,
        data_start,
    }))
}

/// The sizes and checksum after a deflated entry's data, with or without the optional signature.
fn read_descriptor(bytes: &[u8], at: usize) -> Result<Descriptor, String> {
    need(bytes, at + 12)?;
    let first = u32_at(bytes, at)?;
    if first == DESCRIPTOR_SIGNATURE {
        need(bytes, at + 16)?;
        return Ok(Descriptor {
            crc: u32_at(bytes, at + 4)?,
            packed: u32_at(bytes, at + 8)?,
            size: u32_at(bytes, at + 12)?,
            length: 16,
        });
    }
    Ok(Descriptor { crc: first, packed: u32_at(bytes, at + 4)?, size: u32_at(bytes, at + 8)?, length: 12 })
}

/// Inflates a raw deflate stream that starts at `start`. Returns the bytes and how many packed
/// bytes the stream used. Stops with the "too much data" sentence once the output passes `budget`.
fn inflate(bytes: &[u8], start: usize, budget: usize) -> Result<(Vec<u8>, usize), String> {
    let source = &bytes[start..];
    let cap = budget.saturating_add(1).max(1);
    let mut stream = Decompress::new(false);
    let mut out: Vec<u8> = Vec::new();
    let mut chunk = [0u8; CHUNK_BYTES];
    loop {
        let (in_before, out_before) = (stream.total_in(), stream.total_out());
        let status = stream
            .decompress(&source[in_before as usize..], &mut chunk, FlushDecompress::None)
            .map_err(|_| refuse(NOT_A_ZIP))?;
        let produced = (stream.total_out() - out_before) as usize;
        out.extend_from_slice(&chunk[..produced]);
        if out.len() > cap {
            return Err(refuse(TOO_MUCH));
        }
        match status {
            Status::StreamEnd => return Ok((out, stream.total_in() as usize)),
            // No progress with input left over or exhausted means the stream was cut short.
            _ if produced == 0 && stream.total_in() == in_before => return Err(refuse(NOT_A_ZIP)),
            _ => {}
        }
    }
}

/// One entry's bytes, where the entry ends, and the checks the phone's reader makes on its sizes
/// and checksum.
fn read_body(bytes: &[u8], header: &Header, budget: usize) -> Result<(Vec<u8>, usize), String> {
    if header.method == 0 {
        let end = header.data_start + header.size as usize;
        need(bytes, end)?;
        let data = &bytes[header.data_start..end];
        if data.len() > budget {
            return Err(refuse(TOO_MUCH));
        }
        if crc32fast::hash(data) != header.crc {
            return Err(refuse(NOT_A_ZIP));
        }
        return Ok((data.to_vec(), end));
    }
    if header.method != 8 {
        return Err(refuse(NOT_A_ZIP));
    }
    let (data, used) = inflate(bytes, header.data_start, budget)?;
    let declared = if header.flag & 8 != 0 {
        read_descriptor(bytes, header.data_start + used)?
    } else {
        Descriptor { crc: header.crc, packed: header.packed, size: header.size, length: 0 }
    };
    let same = declared.size as usize == data.len()
        && declared.packed as usize == used
        && declared.crc == crc32fast::hash(&data);
    if !same {
        return Err(refuse(NOT_A_ZIP));
    }
    if data.len() > budget {
        return Err(refuse(TOO_MUCH));
    }
    Ok((data, header.data_start + used + declared.length))
}

fn scan(bytes: &[u8]) -> Result<OpenedZip, String> {
    let mut files: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    let mut duplicates = Vec::new();
    let (mut count, mut total, mut at, mut saw_entry) = (0usize, 0usize, 0usize, false);
    while let Some(header) = read_header(bytes, at)? {
        saw_entry = true;
        let is_directory = header.name.ends_with('/');
        if !is_directory {
            count += 1;
            if count > MAX_ENTRIES {
                return Err(too_many_files());
            }
        }
        let (data, next) = read_body(bytes, &header, MAX_TOTAL_BYTES - total)?;
        at = next;
        if is_directory {
            continue;
        }
        total += data.len();
        if data.len() > MAX_ENTRY_BYTES {
            return Err(entry_too_big(&header.name));
        }
        if files.contains_key(&header.name) {
            duplicates.push(header.name);
        } else {
            files.insert(header.name, data);
        }
    }
    if !saw_entry {
        return Err(refuse(NOT_A_ZIP));
    }
    Ok(OpenedZip { files: files.into_iter().collect(), duplicates })
}

/// Opens `bytes` as the phone does. Returns the files in code point order of their names, or the
/// sentence the phone shows when it cannot open the zip. Never panics on any input.
pub fn open_zip(bytes: &[u8]) -> Result<OpenedZip, String> {
    scan(bytes)
}
