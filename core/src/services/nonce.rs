use std::collections::HashSet;

use crate::error::{CoreError, CoreResult};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Nonce(pub String);

#[derive(Debug, Default)]
pub struct NonceIssuer {
    counter: u64,
}

impl NonceIssuer {
    pub fn issue(&mut self) -> Nonce {
        self.counter += 1;
        Nonce(format!("nonce-{}", self.counter))
    }
}

#[derive(Debug, Default)]
pub struct NonceGuard {
    seen: HashSet<String>,
}

impl NonceGuard {
    pub fn consume(&mut self, nonce: &Nonce) -> CoreResult<()> {
        if nonce.0.is_empty() {
            return Err(CoreError::InvalidInput("the nonce is empty".to_string()));
        }
        if !self.seen.insert(nonce.0.clone()) {
            return Err(CoreError::ReplayedNonce(nonce.0.clone()));
        }
        Ok(())
    }
}
