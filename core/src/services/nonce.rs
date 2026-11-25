use std::collections::HashSet;

use rand::RngCore;

use crate::error::{CoreError, CoreResult};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Nonce(pub String);

#[derive(Debug, Default)]
pub struct NonceIssuer;

impl NonceIssuer {
    pub fn new() -> Self {
        NonceIssuer
    }

    pub fn issue(&mut self) -> Nonce {
        let mut bytes = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut bytes);
        let mut text = String::with_capacity(64);
        for byte in bytes {
            text.push_str(&format!("{:02x}", byte));
        }
        Nonce(text)
    }
}

#[derive(Debug, Default)]
pub struct NonceGuard {
    seen: HashSet<String>,
}

impl NonceGuard {
    pub fn consume(&mut self, nonce: &Nonce) -> CoreResult<()> {
        if nonce.0.len() != 64 || !nonce.0.bytes().all(|value| value.is_ascii_hexdigit()) {
            return Err(CoreError::InvalidInput("the nonce is not 256-bit hex".to_string()));
        }
        if !self.seen.insert(nonce.0.clone()) {
            return Err(CoreError::ReplayedNonce(nonce.0.clone()));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nonces_are_256_bit_and_single_use() {
        let mut issuer = NonceIssuer::new();
        let first = issuer.issue();
        let second = issuer.issue();
        assert_eq!(first.0.len(), 64);
        assert_ne!(first, second);
        let mut guard = NonceGuard::default();
        assert!(guard.consume(&first).is_ok());
        assert!(guard.consume(&first).is_err());
        assert!(guard.consume(&Nonce("short".to_string())).is_err());
    }
}
