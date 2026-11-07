//! Builds a zip the way the phone's export does (java.util.zip.ZipOutputStream): deflated entries
//! whose sizes sit in a data descriptor after the data, with the UTF-8 name flag set. A stored
//! variant (sizes in the local header, no descriptor) feeds the readers a second layout of the same
//! files. The time stamp is fixed so equal input gives equal bytes. Ported from `lib/zip-writer.js`.
#![allow(dead_code)]

use flate2::write::DeflateEncoder;
use flate2::Compression;
use std::io::Write;

const LOCAL_SIGNATURE: u32 = 0x04034b50;
const CENTRAL_SIGNATURE: u32 = 0x02014b50;
const END_SIGNATURE: u32 = 0x06054b50;
const DESCRIPTOR_SIGNATURE: u32 = 0x08074b50;
const UTF8_NAME_FLAG: u16 = 0x0800;
const DESCRIPTOR_FLAG: u16 = 0x0008;
const VERSION: u16 = 20;
const DOS_DATE: u16 = 0x21; // 1980-01-01

#[derive(Clone, Copy, PartialEq)]
pub enum Method {
    Deflate,
    Stored,
}

/// One entry to write: the raw bytes of its name and its data (empty for a folder).
pub struct Entry {
    pub name: Vec<u8>,
    pub data: Vec<u8>,
}

struct Written {
    name: Vec<u8>,
    size: usize,
    packed: Vec<u8>,
    crc: u32,
    offset: usize,
}

fn put16(bytes: &mut [u8], at: usize, value: u16) {
    bytes[at..at + 2].copy_from_slice(&value.to_le_bytes());
}

fn put32(bytes: &mut [u8], at: usize, value: u32) {
    bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
}

fn flags(descriptor: bool) -> u16 {
    UTF8_NAME_FLAG | if descriptor { DESCRIPTOR_FLAG } else { 0 }
}

fn local_header(entry: &Written, method: u16, descriptor: bool) -> Vec<u8> {
    let mut header = vec![0u8; 30];
    put32(&mut header, 0, LOCAL_SIGNATURE);
    put16(&mut header, 4, VERSION);
    put16(&mut header, 6, flags(descriptor));
    put16(&mut header, 8, method);
    put16(&mut header, 12, DOS_DATE);
    if !descriptor {
        put32(&mut header, 14, entry.crc);
        put32(&mut header, 18, entry.packed.len() as u32);
        put32(&mut header, 22, entry.size as u32);
    }
    put16(&mut header, 26, entry.name.len() as u16);
    header
}

fn data_descriptor(entry: &Written) -> Vec<u8> {
    let mut tail = vec![0u8; 16];
    put32(&mut tail, 0, DESCRIPTOR_SIGNATURE);
    put32(&mut tail, 4, entry.crc);
    put32(&mut tail, 8, entry.packed.len() as u32);
    put32(&mut tail, 12, entry.size as u32);
    tail
}

fn central_record(entry: &Written, method: u16, descriptor: bool) -> Vec<u8> {
    let mut record = vec![0u8; 46];
    put32(&mut record, 0, CENTRAL_SIGNATURE);
    put16(&mut record, 4, VERSION);
    put16(&mut record, 6, VERSION);
    put16(&mut record, 8, flags(descriptor));
    put16(&mut record, 10, method);
    put16(&mut record, 14, DOS_DATE);
    put32(&mut record, 16, entry.crc);
    put32(&mut record, 20, entry.packed.len() as u32);
    put32(&mut record, 24, entry.size as u32);
    put16(&mut record, 28, entry.name.len() as u16);
    put32(&mut record, 42, entry.offset as u32);
    record.extend_from_slice(&entry.name);
    record
}

fn end_record(count: usize, size: usize, offset: usize) -> Vec<u8> {
    let mut end = vec![0u8; 22];
    put32(&mut end, 0, END_SIGNATURE);
    put16(&mut end, 8, count as u16);
    put16(&mut end, 10, count as u16);
    put32(&mut end, 12, size as u32);
    put32(&mut end, 16, offset as u32);
    end
}

fn deflate(data: &[u8]) -> Vec<u8> {
    let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(data).expect("deflate into memory");
    encoder.finish().expect("deflate into memory")
}

/// The bytes of a zip holding `entries`, written as `method` says.
pub fn build_zip(entries: &[Entry], method: Method) -> Vec<u8> {
    let (code, descriptor) = if method == Method::Deflate { (8, true) } else { (0, false) };
    let mut out: Vec<u8> = Vec::new();
    let mut written: Vec<Written> = Vec::new();
    for entry in entries {
        let packed = if descriptor { deflate(&entry.data) } else { entry.data.clone() };
        let item = Written { name: entry.name.clone(), size: entry.data.len(), packed, crc: crc32fast::hash(&entry.data), offset: out.len() };
        out.extend(local_header(&item, code, descriptor));
        out.extend_from_slice(&item.name);
        out.extend_from_slice(&item.packed);
        if descriptor {
            out.extend(data_descriptor(&item));
        }
        written.push(item);
    }
    let central_start = out.len();
    for item in &written {
        out.extend(central_record(item, code, descriptor));
    }
    let central_size = out.len() - central_start;
    out.extend(end_record(written.len(), central_size, central_start));
    out
}

/// The bytes of a zip holding the named text files, deflated the way the phone writes them.
pub fn zip_of(files: &[(&str, &str)]) -> Vec<u8> {
    let entries: Vec<Entry> = files.iter().map(|(name, text)| Entry { name: name.as_bytes().to_vec(), data: text.as_bytes().to_vec() }).collect();
    build_zip(&entries, Method::Deflate)
}
