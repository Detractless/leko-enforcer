//! Reading the fields of a session rule: `kind` "session" with `endsAt`, a UTC moment, and no
//! `days`. The moment reader is also the one a moment seal uses for its `at`.

use crate::config::strict_fields::{Read, StrictFields};
use super::utc_moment::parse_utc_moment;

/// The moment text of a session end or a seal start, as the file writes it.
pub const MOMENT_PROBLEM: &str = "must be a UTC moment written YYYY-MM-DDTHH:MM:SSZ, such as 2026-11-05T18:30:00Z";

/// The instant, in milliseconds since the epoch, of the moment field `name`.
pub fn read_moment(fields: &mut StrictFields, name: &str) -> Read<i64> {
    let text = fields.text(name)?;
    match parse_utc_moment(&text) {
        Some(instant) => Ok(instant),
        None => fields.fail(name, MOMENT_PROBLEM),
    }
}

/// The fields of a session rule after `id`, `days` and `kind`: its end, and a refusal of any days.
pub fn read_session(fields: &mut StrictFields) -> Read<i64> {
    if fields.has("days") {
        return fields.fail("days", "is not used: a session rule has no weekdays, it ends at its endsAt");
    }
    read_moment(fields, "endsAt")
}
