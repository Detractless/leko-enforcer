//! Native-side glue for mandatory browser extension enforcement.

use std::collections::HashMap;

use leko_core::config_time::zone_time::ZoneTime;
use leko_core::decision::block_cover::Target;
use leko_core::decision::decide_target::decide;
use leko_core::models::specs::ReadSet;
use leko_enforcers_browser::{default_handshake, BrowserGuard, BrowserId, GuardAction, Heartbeat};

/// True when at least one observed browser target is currently locked.
pub fn any_block_locked(set: &ReadSet, now: i64, zone: &ZoneTime, bundle_ids: &[String]) -> bool {
    let protected = crate::enforcement::enforcement_loop::protected();
    bundle_ids.iter().any(|id| {
        let decision = decide(set, Target::App(id.as_str()), now, zone, &protected);
        decision.closed
    })
}

/// Runs one guard tick. Warnings and closes are injected so the core stays pure.
pub fn judge_browsers<W, C>(
    guard: &mut BrowserGuard,
    browsers: &[BrowserId],
    heartbeats: &HashMap<BrowserId, Heartbeat>,
    now_ms: i64,
    block_locked: bool,
    mut warn: W,
    mut close: C,
) where
    W: FnMut(&BrowserId),
    C: FnMut(&BrowserId),
{
    for (browser, action) in guard.tick(&default_handshake(), browsers, heartbeats, now_ms, block_locked) {
        match action {
            GuardAction::Warn => warn(&browser),
            GuardAction::Close => close(&browser),
            GuardAction::None => {}
        }
    }
}
