//! How one lock compares with another. Only the pairs the contract lists tighten; every other
//! change loosens. Ported from `lib/lock-compare.js`.

use crate::config_time::due_time::{due, effective, seal_end, HOUR_MS};
use crate::config_time::window_mask::WindowMask;
use crate::config_time::zone_time::ZoneTime;
use crate::models::specs::Lock;

/// True when the two locks have the same level and the same parameters, whatever their keys.
pub fn same_level(a: &Lock, b: &Lock) -> bool {
    match (a, b) {
        (Lock::Open { .. }, Lock::Open { .. }) => true,
        (Lock::Wait { hours: x, .. }, Lock::Wait { hours: y, .. }) => x == y,
        (Lock::ChangeHours { from: f1, to: t1, hours: h1, .. }, Lock::ChangeHours { from: f2, to: t2, hours: h2, .. }) => f1 == f2 && t1 == t2 && h1 == h2,
        (Lock::SealedUntil { until: x, .. }, Lock::SealedUntil { until: y, .. }) => x == y,
        (Lock::SealedAt { at: x, .. }, Lock::SealedAt { at: y, .. }) => x == y,
        _ => false,
    }
}

fn window_inside(inner: (i64, i64), outer: (i64, i64)) -> bool {
    WindowMask::of(&[inner]).subset_of(&WindowMask::of(&[outer]))
}

/// The instant a date seal or a moment seal ends; `None` for any other lock.
fn seal_ends(lock: &Lock, zone: &ZoneTime) -> Option<i64> {
    match lock {
        Lock::SealedUntil { until, .. } => Some(seal_end(until, zone)),
        Lock::SealedAt { at, .. } => Some(*at),
        _ => None,
    }
}

/// A seal that ends no earlier than the instant the old lock would have let the change land.
fn seal_covers_landing(old: &Lock, ends: i64, requested_g: i64, zone: &ZoneTime) -> bool {
    match due(requested_g, std::slice::from_ref(old), zone) {
        None => true,
        Some(lands) => ends >= lands,
    }
}

fn from_wait(old_hours: i64, old: &Lock, new: &Lock, requested_g: i64, zone: &ZoneTime) -> bool {
    match new {
        Lock::Wait { hours, .. } | Lock::ChangeHours { hours, .. } => *hours >= old_hours,
        Lock::SealedUntil { .. } | Lock::SealedAt { .. } => seal_ends(new, zone).is_some_and(|ends| seal_covers_landing(old, ends, requested_g, zone)),
        Lock::Open { .. } => false,
    }
}

fn from_change_hours(window: (i64, i64), old_hours: i64, old: &Lock, new: &Lock, requested_g: i64, zone: &ZoneTime) -> bool {
    match new {
        Lock::ChangeHours { from, to, hours, .. } => *hours >= old_hours && window_inside((*from, *to), window),
        Lock::SealedUntil { .. } | Lock::SealedAt { .. } => seal_ends(new, zone).is_some_and(|ends| seal_covers_landing(old, ends, requested_g, zone)),
        _ => false,
    }
}

fn from_seal(old_until: &str, new: &Lock, requested_g: i64, zone: &ZoneTime) -> bool {
    match new {
        Lock::SealedUntil { until, .. } => until.as_str() >= old_until,
        Lock::SealedAt { at, .. } => *at >= seal_end(old_until, zone),
        Lock::Wait { hours, .. } | Lock::ChangeHours { hours, .. } => requested_g + hours * HOUR_MS >= seal_end(old_until, zone),
        Lock::Open { .. } => false,
    }
}

/// A moment seal replaced by another lock: it tightens when the new lock lasts at least as long.
fn from_moment(old_at: i64, new: &Lock, requested_g: i64, zone: &ZoneTime) -> bool {
    match new {
        Lock::SealedAt { at, .. } => *at >= old_at,
        Lock::SealedUntil { until, .. } => seal_end(until, zone) >= old_at,
        Lock::Wait { hours, .. } | Lock::ChangeHours { hours, .. } => requested_g + hours * HOUR_MS >= old_at,
        Lock::Open { .. } => false,
    }
}

/// True when changing the level from `old` to `new` at `requested_g` tightens. A seal that has
/// already ended (a date seal or a moment seal) counts as an open lock, so it can be replaced by any lock.
pub fn level_tightens(old: &Lock, new: &Lock, requested_g: i64, zone: &ZoneTime) -> bool {
    let from = effective(old, requested_g, zone);
    if same_level(&from, new) {
        return false;
    }
    match &from {
        Lock::Open { .. } => true,
        Lock::Wait { hours, .. } => from_wait(*hours, &from, new, requested_g, zone),
        Lock::ChangeHours { from: start, to, hours, .. } => from_change_hours((*start, *to), *hours, &from, new, requested_g, zone),
        Lock::SealedUntil { until, .. } => from_seal(until, new, requested_g, zone),
        Lock::SealedAt { at, .. } => from_moment(*at, new, requested_g, zone),
    }
}
