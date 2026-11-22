//! Pure contracts for mandatory browser enforcement.
//!
//! No OS calls and no clock reads here: callers inject observations and time.

use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExtensionStatus {
    Healthy,
    Missing,
    Disabled,
    Detached,
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct BrowserId {
    pub pid: i32,
    pub bundle_id: String,
    pub profile: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Heartbeat {
    pub seen_at_ms: i64,
    pub block_id: String,
    pub attached: bool,
}

pub trait ExtensionHandshake {
    fn status(
        &self,
        browser: &BrowserId,
        heartbeats: &HashMap<BrowserId, Heartbeat>,
        now_ms: i64,
        stale_after_ms: i64,
    ) -> ExtensionStatus;
}

pub trait EnforcementAdapter {
    fn warn(&mut self, browser: &BrowserId);
    fn close(&mut self, browser: &BrowserId);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GuardAction {
    None,
    Warn,
    Close,
}

#[derive(Clone, Copy, Debug)]
pub struct GuardConfig {
    pub stale_after_ms: i64,
    pub grace_ms: i64,
}

impl Default for GuardConfig {
    fn default() -> Self {
        Self { stale_after_ms: 5_000, grace_ms: 60_000 }
    }
}

/// Default handshake: a recent, attached heartbeat is healthy. Anything else
/// is treated as unhealthy; unknown observations never count as healthy.
pub struct DefaultHandshake;

impl ExtensionHandshake for DefaultHandshake {
    fn status(
        &self,
        browser: &BrowserId,
        heartbeats: &HashMap<BrowserId, Heartbeat>,
        now_ms: i64,
        stale_after_ms: i64,
    ) -> ExtensionStatus {
        let Some(beat) = heartbeats.get(browser) else {
            return ExtensionStatus::Missing;
        };
        if !beat.attached {
            return ExtensionStatus::Disabled;
        }
        if now_ms.saturating_sub(beat.seen_at_ms) > stale_after_ms {
            return ExtensionStatus::Detached;
        }
        ExtensionStatus::Healthy
    }
}

/// Pure decision per browser. First unhealthy tick warns; sustained unhealthy
/// ticks past the grace period close the browser, but only while a block is locked.
pub fn judge_guard<H: ExtensionHandshake>(
    handshake: &H,
    browsers: &[BrowserId],
    heartbeats: &HashMap<BrowserId, Heartbeat>,
    now_ms: i64,
    block_locked: bool,
    first_unhealthy: &mut HashMap<BrowserId, i64>,
    config: &GuardConfig,
) -> Vec<(BrowserId, GuardAction)> {
    let mut actions = Vec::new();
    for browser in browsers {
        let status = handshake.status(browser, heartbeats, now_ms, config.stale_after_ms);
        if status == ExtensionStatus::Healthy || !block_locked {
            first_unhealthy.remove(browser);
            actions.push((browser.clone(), GuardAction::None));
            continue;
        }
        let since = *first_unhealthy.entry(browser.clone()).or_insert(now_ms);
        let action = if now_ms.saturating_sub(since) >= config.grace_ms {
            GuardAction::Close
        } else {
            GuardAction::Warn
        };
        actions.push((browser.clone(), action));
    }
    actions
}
