use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoreError {
    Io(String),
    Parse(String),
    UnknownVersion(u32),
    HashMismatch(String),
    MissingAddon(String),
    ReplayedNonce(String),
    InvalidInput(String),
    Clock(String),
}

impl fmt::Display for CoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CoreError::Io(message) => write!(f, "io error: {}", message),
            CoreError::Parse(message) => write!(f, "cannot parse: {}", message),
            CoreError::UnknownVersion(version) => {
                write!(f, "unknown native store version {}", version)
            }
            CoreError::HashMismatch(addon_id) => {
                write!(f, "add-on {} does not match its pinned hash", addon_id)
            }
            CoreError::MissingAddon(addon_id) => write!(f, "add-on {} is missing", addon_id),
            CoreError::ReplayedNonce(nonce) => write!(f, "nonce {} was already used", nonce),
            CoreError::InvalidInput(message) => write!(f, "invalid input: {}", message),
            CoreError::Clock(message) => write!(f, "clock error: {}", message),
        }
    }
}

impl std::error::Error for CoreError {}

pub type CoreResult<T> = Result<T, CoreError>;
