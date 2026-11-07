//! A moment written on a zone's wall clock, with the offset in force at that moment (AC-005): the
//! command's sentences show local time, so the text must be right on the days the offset changes and
//! in zones whose offset is not a whole hour.

use chrono::DateTime;
use leko_core::config_time::zone_time::ZoneTime;

fn moment(text: &str) -> i64 {
    DateTime::parse_from_rfc3339(text).unwrap_or_else(|_| panic!("{text} is not a time")).timestamp_millis()
}

fn zone(name: &str) -> ZoneTime {
    ZoneTime::new(name).unwrap_or_else(|sentence| panic!("{sentence}"))
}

fn text(zone_name: &str, at: &str) -> String {
    zone(zone_name).local_text(moment(at))
}

#[test]
fn a_moment_is_written_on_the_wall_clock_with_the_offset_in_force_then() {
    // Winter and summer offsets of one zone, and a zero offset written Z.
    assert_eq!(text("America/New_York", "2026-11-05T15:00:00Z"), "2026-11-05T10:00:00-05:00");
    assert_eq!(text("America/New_York", "2026-07-01T12:00:00Z"), "2026-07-01T08:00:00-04:00");
    assert_eq!(text("UTC", "2026-11-05T15:00:07Z"), "2026-11-05T15:00:07Z", "seconds are kept");
    assert_eq!(text("Europe/London", "2026-01-15T09:30:00Z"), "2026-01-15T09:30:00Z");
    assert_eq!(text("Europe/London", "2026-07-15T09:30:00Z"), "2026-07-15T10:30:00+01:00");

    // The part of a second is cut off, also before 1970.
    assert_eq!(text("UTC", "2026-11-05T15:00:07.999Z"), "2026-11-05T15:00:07Z");
    assert_eq!(text("UTC", "1969-12-31T23:59:59.500Z"), "1969-12-31T23:59:59Z");

    // The date moves with the zone.
    assert_eq!(text("Asia/Tokyo", "2026-11-05T20:00:00Z"), "2026-11-06T05:00:00+09:00");
    assert_eq!(text("Pacific/Honolulu", "2026-11-05T05:00:00Z"), "2026-11-04T19:00:00-10:00");

    // Half-hour and quarter-hour offsets keep their minutes, and a negative one keeps its sign.
    assert_eq!(text("Asia/Kolkata", "2026-11-05T15:00:00Z"), "2026-11-05T20:30:00+05:30");
    assert_eq!(text("Asia/Kathmandu", "2026-11-05T15:00:00Z"), "2026-11-05T20:45:00+05:45");
    assert_eq!(text("Pacific/Marquesas", "2026-11-05T15:00:00Z"), "2026-11-05T05:30:00-09:30");
    assert_eq!(text("Australia/Lord_Howe", "2026-01-15T00:00:00Z"), "2026-01-15T11:00:00+11:00");
    assert_eq!(text("Australia/Lord_Howe", "2026-07-15T00:00:00Z"), "2026-07-15T10:30:00+10:30");

    // The repeated hour of autumn shows the same wall minutes twice, at two offsets; the hour of
    // spring is skipped by the jump.
    assert_eq!(text("America/New_York", "2026-11-01T05:30:00Z"), "2026-11-01T01:30:00-04:00");
    assert_eq!(text("America/New_York", "2026-11-01T06:30:00Z"), "2026-11-01T01:30:00-05:00");
    assert_eq!(text("America/New_York", "2026-03-08T06:59:59Z"), "2026-03-08T01:59:59-05:00");
    assert_eq!(text("America/New_York", "2026-03-08T07:00:00Z"), "2026-03-08T03:00:00-04:00");

    // Before 1883 New York kept local mean time, -04:56:02: the seconds of the offset move the seconds
    // of the wall clock, and the offset is written to the minute.
    assert_eq!(text("America/New_York", "1850-01-01T12:00:00Z"), "1850-01-01T07:03:58-04:56");

    // A moment no date can hold is written as 1970-01-01 in UTC with Z, and does not crash.
    for extreme in [i64::MAX, i64::MIN, i64::MAX - 1, i64::MIN + 1] {
        for name in ["UTC", "America/New_York", "Asia/Kolkata"] {
            assert_eq!(zone(name).local_text(extreme), "1970-01-01T00:00:00Z", "{name} at {extreme}");
        }
    }
}

#[test]
fn the_text_reads_back_as_the_same_second_in_every_zone_all_year() {
    // An independent check: read the text back with chrono; it must name the same second.
    let start = moment("2026-01-01T00:00:00Z");
    let step = (67 * 60 + 13) * 1000;
    for name in ["America/New_York", "Asia/Kolkata", "Pacific/Marquesas", "Australia/Lord_Howe", "Europe/London", "Asia/Tokyo", "UTC", "America/St_Johns"] {
        let zone = zone(name);
        let mut at = start;
        while at < start + 366 * 86_400_000 {
            let written = zone.local_text(at);
            let read_back = DateTime::parse_from_rfc3339(&written).unwrap_or_else(|_| panic!("{name}: {written} is not a time"));
            assert_eq!(read_back.timestamp(), at.div_euclid(1000), "{name} at {at}: {written}");
            at += step;
        }
    }
}
