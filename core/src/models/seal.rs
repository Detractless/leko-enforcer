use serde::{Deserialize, Serialize};

use crate::models::schedule::Window;
use crate::models::verifier_spec::CustomVerifierSpec;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TypingMode {
    ExactText,
    RandomText,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct QueuedEdit {
    pub key: String,
    pub value: String,
    pub requested_at: String,
    pub cancelled: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Seal {
    NoSeal,
    CountdownSeal {
        wait_seconds: u64,
        unseal_requested_at: Option<String>,
    },
    QueuedDelaySeal {
        delay_seconds: u64,
        pending_edits: Vec<QueuedEdit>,
    },
    TypingSeal {
        mode: TypingMode,
        length: u32,
    },
    TaskSeal {
        task_spec: String,
    },
    EditWindowSeal {
        days: Vec<u32>,
        windows: Vec<Window>,
    },
    ActiveScheduleSeal {
        buffer_minutes: u32,
    },
    PartnerPasswordSeal {
        password_hash: String,
    },
    CustomVerifier {
        spec: CustomVerifierSpec,
        fallback_seal: Option<Box<Seal>>,
    },
}

impl Default for Seal {
    fn default() -> Self {
        Seal::NoSeal
    }
}
