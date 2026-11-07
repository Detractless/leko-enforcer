//! When a loosening requested at one instant may land under a set of locks: the first instant that
//! is no earlier than the end of every wait and seal and lies inside every change window. Waits are
//! elapsed hours; a seal ends at the first midnight after its date in the pinned zone, a moment
//! seal at its exact UTC moment; the search for a window looks nine days ahead. Instants are
//! milliseconds since the epoch. This module never reads the clock: the moment of the request is an
//! argument. Ported from `lib/due-time.js`.

use super::window_mask::WindowMask;
use super::zone_time::{Date, ZoneTime};
use crate::models::specs::Lock;

pub const HOUR_MS: i64 = 3_600_000;
const SEARCH_DAYS: i64 = 9;

/// The instant a seal ends: the first midnight after its date. A date that cannot be read is a
/// seal that never ends; the readers refuse such a date before a lock is ever built.
pub fn seal_end(until: &str, zone: &ZoneTime) -> i64 {
    Date::parse_iso(until).map_or(i64::MAX, |date| zone.instant_of(zone.plus_days(date, 1), 0))
}

/// A seal that has ended acts as an open lock; any other lock is returned as it is.
pub fn effective(lock: &Lock, at_g: i64, zone: &ZoneTime) -> Lock {
    match lock {
        Lock::SealedUntil { until, key } if seal_end(until, zone) <= at_g => Lock::Open { key: key.clone() },
        Lock::SealedAt { at, key } if *at <= at_g => Lock::Open { key: key.clone() },
        other => other.clone(),
    }
}

fn first_inside_mask(from: i64, mask: &WindowMask, zone: &ZoneTime) -> Option<i64> {
    let first_date = zone.date_at(from);
    for offset in 0..SEARCH_DAYS {
        let date = zone.plus_days(first_date, offset);
        for (first, last) in mask.runs() {
            let start = zone.instant_of(date, first);
            let end = zone.instant_of(date, last + 1);
            if end <= start {
                continue;
            }
            if from < end {
                return Some(from.max(start));
            }
        }
    }
    None
}

/// The first instant at or after `requested_g` that is no earlier than the end of every wait and
/// seal in `locks` and lies inside every change window, or `None` when the windows share no minute
/// and the change cannot land by itself.
pub fn due(requested_g: i64, locks: &[Lock], zone: &ZoneTime) -> Option<i64> {
    let mut earliest = requested_g;
    let mut windows: Vec<WindowMask> = Vec::new();
    for lock in locks {
        match lock {
            Lock::Wait { hours, .. } => earliest = earliest.max(requested_g + hours * HOUR_MS),
            Lock::ChangeHours { from, to, hours, .. } => {
                earliest = earliest.max(requested_g + hours * HOUR_MS);
                windows.push(WindowMask::of(&[(*from, *to)]));
            }
            Lock::SealedUntil { until, .. } => earliest = earliest.max(seal_end(until, zone)),
            Lock::SealedAt { at, .. } => earliest = earliest.max(*at),
            Lock::Open { .. } => {}
        }
    }
    let Some(first) = windows.first() else { return Some(earliest) };
    let common = windows.iter().skip(1).fold(first.clone(), |all, next| all.intersect(next));
    if common.is_empty() {
        None
    } else {
        first_inside_mask(earliest, &common, zone)
    }
}
