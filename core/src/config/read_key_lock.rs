//! Reads the partner key and the lock that a settings file can carry, with the phone's order of
//! reads and its sentences. Ported from Enforcer's `lib/read-key-lock.js`.

use super::strict_fields::{Read, StrictFields};
use crate::models::specs::{Key, Lock, ReadMode};
use crate::sessions::session_rule::read_moment;

pub const MIN_ITERATIONS: i64 = 100_000;
pub const MAX_ITERATIONS: i64 = 600_000;
pub const MAX_HOURS: i64 = 8760;
pub const ALG_PBKDF2: &str = "pbkdf2-sha256";
pub const ALG_SHA256: &str = "sha256";

fn read_hash(fields: &mut StrictFields) -> Read<String> {
    let hash = fields.text("hash")?;
    if hash.len() != 64 || !hash.bytes().all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c)) {
        return fields.fail("hash", "must be 64 lowercase hex characters");
    }
    Ok(hash)
}

/// Base64 of 16 bytes: 22 characters of the alphabet, then "==".
fn is_salt(salt: &str) -> bool {
    let b = salt.as_bytes();
    b.len() == 24 && &b[22..] == b"==" && b[..22].iter().all(|c| c.is_ascii_alphanumeric() || *c == b'+' || *c == b'/')
}

/// A key object as the file writes it: `{alg, iterations, salt, hash}` or the old `{alg: "sha256", hash}`.
pub fn read_key(fields: &mut StrictFields) -> Read<Key> {
    let alg = fields.text("alg")?;
    let key = if alg == ALG_PBKDF2 {
        let iterations = fields.whole("iterations", MIN_ITERATIONS, MAX_ITERATIONS)?;
        let salt = fields.text("salt")?;
        if !is_salt(&salt) {
            return fields.fail("salt", "must be base64 of 16 bytes");
        }
        Key { alg, iterations: Some(iterations), salt: Some(salt), hash: read_hash(fields)? }
    } else if alg == ALG_SHA256 {
        Key { alg, iterations: None, salt: None, hash: read_hash(fields)? }
    } else {
        return fields.fail("alg", &format!("must be \"{ALG_PBKDF2}\" or \"{ALG_SHA256}\""));
    };
    fields.finish()?;
    Ok(key)
}

fn read_partner_key(fields: &mut StrictFields) -> Read<Option<Key>> {
    match fields.child("partnerKeyHash")? {
        Some(mut child) => Ok(Some(read_key(&mut child)?)),
        None => Ok(None),
    }
}

fn read_change_hours(fields: &mut StrictFields) -> Read<Lock> {
    let from = fields.clock("from", false)?;
    let to = fields.clock("to", true)?;
    if from == to {
        return fields.fail("to", "must differ from \"from\"");
    }
    let hours = fields.whole_opt("hours", 0, MAX_HOURS)?.unwrap_or(0);
    Ok(Lock::ChangeHours { from, to, hours, key: read_partner_key(fields)? })
}

/// The lock object of a file; a missing lock is open.
pub fn read_lock(parent: &mut StrictFields) -> Read<Lock> {
    let Some(mut fields) = parent.child("lock")? else {
        return Ok(Lock::open());
    };
    let level = fields.text("level")?;
    let lock = match level.as_str() {
        "open" => Lock::Open { key: read_partner_key(&mut fields)? },
        "wait" => {
            let hours = fields.whole("hours", 1, MAX_HOURS)?;
            Lock::Wait { hours, key: read_partner_key(&mut fields)? }
        }
        "changeHours" => read_change_hours(&mut fields)?,
        "sealedUntil" => {
            let until = fields.date("until")?;
            Lock::SealedUntil { until, key: read_partner_key(&mut fields)? }
        }
        "sealedAt" if fields.mode() == ReadMode::Extended => {
            let at = read_moment(&mut fields, "at")?;
            Lock::SealedAt { at, key: read_partner_key(&mut fields)? }
        }
        _ => return fields.fail("level", "must be open, wait, changeHours or sealedUntil"),
    };
    fields.finish()?;
    Ok(lock)
}
