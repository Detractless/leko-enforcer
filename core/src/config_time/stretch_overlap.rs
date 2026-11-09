//! Tells whether two allowed-hour stretches share a minute of the 24-hour circle.

use super::zone_time::MINUTES_PER_DAY;

/// True when two stretches share a minute of the 24-hour circle.
pub fn stretches_overlap(stretches: &[(i64, i64)]) -> bool {
    let mut covered = vec![false; MINUTES_PER_DAY as usize];
    for &(from, to) in stretches {
        let mut minute = from;
        while minute != to % MINUTES_PER_DAY {
            if covered[minute as usize] {
                return true;
            }
            covered[minute as usize] = true;
            minute = (minute + 1) % MINUTES_PER_DAY;
        }
    }
    false
}
