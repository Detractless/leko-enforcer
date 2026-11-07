//! The moment text and the instant it names agree with the calendar for every year 0 to 9999, and
//! the dates and times the form forbids are refused. (chrono is the reference here, not in the library.)
use chrono::{DateTime, Datelike, Utc};
use leko_core::sessions::utc_moment::{format_utc_moment, parse_utc_moment};

#[test]
fn moments_match_the_calendar_in_every_year_from_0_to_9999() {
    let mut instant: i64 = -62_167_219_200_000; // 0000-01-01
    let end: i64 = 253_402_300_800_000; // 10000-01-01
    let mut checked = 0u64;
    while instant < end {
        let want = DateTime::<Utc>::from_timestamp_millis(instant).unwrap().format("%Y-%m-%dT%H:%M:%SZ").to_string();
        let got = format_utc_moment(instant).expect("in range");
        assert_eq!(got, want, "format at {instant}");
        assert_eq!(parse_utc_moment(&got), Some(instant), "parse of {got}");
        instant += 86_400_000 * 7 + 3_599_000 + 13_000; // odd step so hours and seconds vary
        checked += 1;
    }
    assert!(checked > 40_000);
    assert_eq!(format_utc_moment(end), None);
    assert_eq!(format_utc_moment(-62_167_219_200_001), None);
    assert!(DateTime::<Utc>::from_timestamp_millis(end).unwrap().year() == 10000);
    for bad in ["2026-02-29T00:00:00Z", "2100-02-29T00:00:00Z", "2026-13-01T00:00:00Z", "2026-00-10T00:00:00Z", "2026-04-31T00:00:00Z", "2026-06-31T00:00:00Z", "2026-09-31T00:00:00Z", "2026-11-31T00:00:00Z", "2026-01-01T24:00:00Z", "2026-01-01T00:60:00Z", "2026-01-01T00:00:60Z"] {
        assert_eq!(parse_utc_moment(bad), None, "{bad}");
    }
    assert!(parse_utc_moment("2024-02-29T23:59:59Z").is_some());
    assert!(parse_utc_moment("2000-02-29T00:00:00Z").is_some());
}
