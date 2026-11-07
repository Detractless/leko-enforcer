//! The next moment at which anything could change: the first instant after the given one at which
//! the closing state of any rule of an on block differs from what it is now. It looks at every whole
//! minute for eight days (which finds stretch edges, day starts and clock-change jumps without copying
//! any zone rule) and at the end of every session. A caller sleeps until this moment, then asks again.

use super::rule_closes::{rule_state, LocalMoment, RuleState};
use crate::config_time::zone_time::{ZoneTime, MINUTES_PER_DAY};
use crate::models::specs::{ReadSet, Rule};

/// How far ahead the search looks. More than a week, so a rule on one weekday is always seen again.
pub const LOOK_AHEAD_DAYS: i64 = 8;
const MINUTE_MS: i64 = 60_000;

fn states(rules: &[&Rule], now: &LocalMoment) -> Vec<RuleState> {
    rules.iter().map(|rule| rule_state(rule, now)).collect()
}

/// The first instant after `at_ms`, within eight days, at which `view` of the moment differs from
/// `view` at `at_ms`: looked at every whole minute and at the end of every session among `rules`
/// (a session end counts only when the view differs there). `None` when nothing differs in that
/// time, or when the look-ahead would pass the largest moment that can be counted.
pub fn first_difference<T: PartialEq>(rules: &[&Rule], at_ms: i64, zone: &ZoneTime, view: impl Fn(&LocalMoment) -> T) -> Option<i64> {
    let steps = LOOK_AHEAD_DAYS * MINUTES_PER_DAY;
    let horizon = at_ms.checked_add(steps * MINUTE_MS)?;
    let first_minute = at_ms.div_euclid(MINUTE_MS).checked_add(1)?.checked_mul(MINUTE_MS)?;
    let before = view(&LocalMoment::new(at_ms, zone));
    let differs = |moment: i64| view(&LocalMoment::new(moment, zone)) != before;
    let by_minute = (0..steps).map_while(|step| first_minute.checked_add(step * MINUTE_MS)).find(|&moment| differs(moment));
    let session_end = rules.iter().filter(|rule| rule.kind == "session").filter_map(|rule| rule.ends_at).filter(|&end| end > at_ms && end <= horizon && differs(end)).min();
    by_minute.into_iter().chain(session_end).min()
}

/// The first instant after `at_ms`, within eight days, at which a rule of an on block opens or
/// closes; `None` when nothing changes in that time, or when the look-ahead would pass the largest
/// moment that can be counted.
pub fn next_change(set: &ReadSet, at_ms: i64, zone: &ZoneTime) -> Option<i64> {
    let rules: Vec<&Rule> = set.blocks.values().filter(|block| block.on).flat_map(|block| block.rules.iter()).collect();
    first_difference(&rules, at_ms, zone, |now| states(&rules, now))
}
