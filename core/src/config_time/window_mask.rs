//! A set of minutes of the day, one flag for each of the 1440 minutes, read on the 24-hour circle.
//! A stretch whose start is after its end crosses midnight, and an end of 1440 (24:00) runs to the
//! end of the day. Ported from `lib/window-mask.js`.

pub const PER_DAY: usize = 1440;

#[derive(Debug, Clone, PartialEq)]
pub struct WindowMask {
    bits: Vec<bool>,
}

impl WindowMask {
    /// The mask of a list of stretches, each (from, to) in minutes after midnight.
    pub fn of(stretches: &[(i64, i64)]) -> WindowMask {
        let mut bits = vec![false; PER_DAY];
        let day = PER_DAY as i64;
        for &(from, to) in stretches {
            let end = to.rem_euclid(day);
            let mut minute = from.rem_euclid(day);
            for guard in 1..=PER_DAY + 1 {
                if minute == end && guard > 1 {
                    break;
                }
                bits[minute as usize] = true;
                minute = (minute + 1) % day;
                if minute == end {
                    break;
                }
            }
        }
        WindowMask { bits }
    }

    pub fn is_empty(&self) -> bool {
        !self.bits.iter().any(|on| *on)
    }

    pub fn contains(&self, minute: i64) -> bool {
        self.bits[minute.rem_euclid(PER_DAY as i64) as usize]
    }

    pub fn subset_of(&self, other: &WindowMask) -> bool {
        self.bits.iter().zip(&other.bits).all(|(on, there)| !on || *there)
    }

    pub fn intersect(&self, other: &WindowMask) -> WindowMask {
        WindowMask { bits: self.bits.iter().zip(&other.bits).map(|(a, b)| *a && *b).collect() }
    }

    /// The runs of consecutive minutes inside the day as (first, last), in order; a run that wraps midnight appears as two.
    pub fn runs(&self) -> Vec<(i64, i64)> {
        let mut found = Vec::new();
        let mut start: Option<usize> = None;
        for minute in 0..=PER_DAY {
            let on = minute < PER_DAY && self.bits[minute];
            match (on, start) {
                (true, None) => start = Some(minute),
                (false, Some(first)) => {
                    found.push((first as i64, minute as i64 - 1));
                    start = None;
                }
                _ => {}
            }
        }
        found
    }
}
