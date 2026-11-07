//! Writes named files as a zip: every file stored (not packed) with its sizes and checksum in its
//! local header, the UTF-8 name flag set, a central directory, and a fixed time stamp so equal input
//! gives equal bytes. The core's reader, the phone's `ZipInputStream` and ordinary unzip tools all
//! read this layout. A list the core's own reader would refuse is refused here, with the same sentence,
//! so a zip is never written that cannot be read back. Makes no operating system calls.

use super::zip_reader::{entry_too_big, too_many_files, MAX_ENTRIES, MAX_ENTRY_BYTES, MAX_TOTAL_BYTES, NOT_A_ZIP, TOO_MUCH};
use std::collections::HashSet;

const LOCAL_SIGNATURE: u32 = 0x0403_4b50;
const CENTRAL_SIGNATURE: u32 = 0x0201_4b50;
const END_SIGNATURE: u32 = 0x0605_4b50;
const UTF8_NAME_FLAG: u16 = 0x0800;
const VERSION: u16 = 20;
/// 1980-01-01, the first date a zip can hold.
const DOS_DATE: u16 = 0x21;
const MAX_NAME_BYTES: usize = 1024;

fn put16(out: &mut Vec<u8>, value: u16) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn put32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

/// Refuses the list with the reader's sentence when the reader would refuse the zip it makes.
fn check_list(files: &[(String, Vec<u8>)]) -> Result<(), String> {
    if files.is_empty() {
        return Err(NOT_A_ZIP.to_string());
    }
    let (mut total, mut names) = (0usize, HashSet::new());
    for (index, (name, data)) in files.iter().enumerate() {
        if name.is_empty() || name.ends_with('/') || name.len() > MAX_NAME_BYTES {
            return Err(format!("The entry name \"{name}\" cannot be written."));
        }
        if !names.insert(name.as_str()) {
            return Err(format!("The entry \"{name}\" appears twice."));
        }
        if index + 1 > MAX_ENTRIES {
            return Err(too_many_files());
        }
        total += data.len();
        if total > MAX_TOTAL_BYTES {
            return Err(TOO_MUCH.to_string());
        }
        if data.len() > MAX_ENTRY_BYTES {
            return Err(entry_too_big(name));
        }
    }
    Ok(())
}

fn local_header(out: &mut Vec<u8>, name: &str, data: &[u8], crc: u32) {
    put32(out, LOCAL_SIGNATURE);
    put16(out, VERSION);
    put16(out, UTF8_NAME_FLAG);
    put16(out, 0); // stored
    put16(out, 0); // time
    put16(out, DOS_DATE);
    put32(out, crc);
    put32(out, data.len() as u32);
    put32(out, data.len() as u32);
    put16(out, name.len() as u16);
    put16(out, 0); // no extra field
    out.extend_from_slice(name.as_bytes());
}

fn central_header(out: &mut Vec<u8>, name: &str, data: &[u8], crc: u32, offset: usize) {
    put32(out, CENTRAL_SIGNATURE);
    put16(out, VERSION);
    put16(out, VERSION);
    put16(out, UTF8_NAME_FLAG);
    put16(out, 0);
    put16(out, 0);
    put16(out, DOS_DATE);
    put32(out, crc);
    put32(out, data.len() as u32);
    put32(out, data.len() as u32);
    put16(out, name.len() as u16);
    for _ in 0..4 {
        put16(out, 0); // extra field, comment, disk number, internal attributes
    }
    put32(out, 0); // external attributes
    put32(out, offset as u32);
    out.extend_from_slice(name.as_bytes());
}

/// The bytes of a zip holding `files` (name, content) in the order given, or the sentence saying why
/// the list cannot be written. An empty list is refused, as the reader refuses a zip with no entry.
pub fn write_zip(files: &[(String, Vec<u8>)]) -> Result<Vec<u8>, String> {
    check_list(files)?;
    let (mut out, mut central) = (Vec::new(), Vec::new());
    for (name, data) in files {
        let crc = crc32fast::hash(data);
        central_header(&mut central, name, data, crc, out.len());
        local_header(&mut out, name, data, crc);
        out.extend_from_slice(data);
    }
    let (start, size) = (out.len(), central.len());
    out.extend(central);
    put32(&mut out, END_SIGNATURE);
    put16(&mut out, 0);
    put16(&mut out, 0);
    put16(&mut out, files.len() as u16);
    put16(&mut out, files.len() as u16);
    put32(&mut out, size as u32);
    put32(&mut out, start as u32);
    put16(&mut out, 0);
    Ok(out)
}
