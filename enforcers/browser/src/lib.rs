//! Browser-specific wrapper around the pure mandatory-enforcement contracts.

use std::collections::HashMap;

pub use leko_core::enforcement::{
    judge_guard, BrowserId, EnforcementAdapter, ExtensionHandshake, ExtensionStatus,
    GuardAction, GuardConfig, Heartbeat,
};
use leko_core::enforcement::DefaultHandshake;

/// Holds grace-period state between tick-loop calls. No clock or OS access.
pub struct BrowserGuard {
    first_unhealthy: HashMap<BrowserId, i64>,
    config: GuardConfig,
}

impl BrowserGuard {
    pub fn new(config: GuardConfig) -> Self {
        Self { first_unhealthy: HashMap::new(), config }
    }

    pub fn tick<H: ExtensionHandshake>(
        &mut self,
        handshake: &H,
        browsers: &[BrowserId],
        heartbeats: &HashMap<BrowserId, Heartbeat>,
        now_ms: i64,
        block_locked: bool,
    ) -> Vec<(BrowserId, GuardAction)> {
        judge_guard(
            handshake,
            browsers,
            heartbeats,
            now_ms,
            block_locked,
            &mut self.first_unhealthy,
            &self.config,
        )
    }
}

impl Default for BrowserGuard {
    fn default() -> Self {
        Self::new(GuardConfig::default())
    }
}

/// Convenience constructor for callers that use the default heartbeat rules.
pub fn default_guard() -> BrowserGuard {
    BrowserGuard::default()
}

pub fn default_handshake() -> DefaultHandshake {
    DefaultHandshake
}
