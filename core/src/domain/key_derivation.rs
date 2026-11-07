//! A partner key as the phone checks it: the secret is normalised to Unicode NFC, encoded as UTF-8
//! and run through PBKDF2 with HMAC-SHA-256 for 32 bytes. This module never writes a secret anywhere
//! and never panics on bad input. Ported from `lib/key-derivation.js`.

use crate::models::specs::Key;
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use pbkdf2::pbkdf2_hmac;
use sha2::Sha256;
use unicode_normalization::UnicodeNormalization;

pub const ALG: &str = "pbkdf2-sha256";
pub const MIN_ITERATIONS: i64 = 100_000;
pub const MAX_ITERATIONS: i64 = 600_000;
pub const SALT_BYTES: usize = 16;
pub const HASH_BYTES: usize = 32;

/// Why a hash could not be worked out.
#[derive(Debug, Clone, PartialEq)]
pub enum KeyError {
    EmptySecret,
    BadSalt,
    BadIterations,
    NotPbkdf2,
}

impl KeyError {
    pub fn sentence(&self) -> String {
        match self {
            KeyError::EmptySecret => "the secret is empty".to_string(),
            KeyError::BadSalt => format!("the salt must be {SALT_BYTES} bytes"),
            KeyError::BadIterations => format!("the iteration count must be a whole number from {MIN_ITERATIONS} to {MAX_ITERATIONS}"),
            KeyError::NotPbkdf2 => "this key is not a salted PBKDF2 key".to_string(),
        }
    }
}

/// The digest of `secret` with `salt` (16 bytes) and `iterations`, as 64 lowercase hex characters.
pub fn derive_hash(secret: &str, salt: &[u8], iterations: i64) -> Result<String, KeyError> {
    let normal: String = secret.nfc().collect();
    if normal.is_empty() {
        return Err(KeyError::EmptySecret);
    }
    if salt.len() != SALT_BYTES {
        return Err(KeyError::BadSalt);
    }
    if !(MIN_ITERATIONS..=MAX_ITERATIONS).contains(&iterations) {
        return Err(KeyError::BadIterations);
    }
    let mut out = [0u8; HASH_BYTES];
    pbkdf2_hmac::<Sha256>(normal.as_bytes(), salt, iterations as u32, &mut out);
    Ok(out.iter().map(|byte| format!("{byte:02x}")).collect())
}

/// A key object for `secret` with the given salt and iteration count.
pub fn create_key(secret: &str, salt: &[u8], iterations: i64) -> Result<Key, KeyError> {
    let hash = derive_hash(secret, salt, iterations)?;
    Ok(Key { alg: ALG.to_string(), iterations: Some(iterations), salt: Some(STANDARD.encode(salt)), hash })
}

/// Equal bytes, compared without stopping at the first difference.
fn same_bytes(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |seen, (x, y)| seen | (x ^ y)) == 0
}

/// True when `secret` gives the hash of the PBKDF2 key `key`.
pub fn matches_key(secret: &str, key: &Key) -> Result<bool, KeyError> {
    let (Some(salt), Some(iterations)) = (key.salt.as_deref(), key.iterations) else { return Err(KeyError::NotPbkdf2) };
    if key.alg != ALG {
        return Err(KeyError::NotPbkdf2);
    }
    let salt = STANDARD.decode(salt).map_err(|_| KeyError::BadSalt)?;
    let computed = derive_hash(secret, &salt, iterations)?;
    Ok(same_bytes(computed.as_bytes(), key.hash.as_bytes()))
}
