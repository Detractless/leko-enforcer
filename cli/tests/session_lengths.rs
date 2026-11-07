//! A length is whole minutes or one whole number with m, h or d, from 1 minute to 7 days after
//! conversion (AC-002). Outside that, or too big to count, the core refuses it: exit 1, its sentence
//! on stderr, nothing on stdout. Text that is not a length at all is a usage problem (AC-003).

mod run_capture;

use leko_cli::session::session_length::{count_of, minutes_of, read_length};
use run_capture::{ends_in_json, expected_end, moment, run_at};

#[test]
fn lengths_in_minutes_hours_and_days_stay_within_one_minute_to_seven_days() {
    let now = moment("2026-11-05T10:00:00Z") + 250;

    let good = [
        ("1", 1),
        ("1m", 1),
        ("90", 90),
        ("90m", 90),
        ("2h", 120),
        ("24h", 1_440),
        ("1d", 1_440),
        ("7d", 10_080),
        ("168h", 10_080),
        ("10080", 10_080),
        ("10080m", 10_080),
        ("007", 7),
        ("0090m", 90),
    ];
    for (text, minutes) in good {
        let made = run_at(&["session", "--json", text], now);
        assert_eq!(made.code, 0, "{text}: {}", made.err);
        assert_eq!(ends_in_json(&made), expected_end(now, minutes), "{text} is {minutes} minutes");
        assert_eq!(read_length(text), Some(minutes), "{text}");
    }

    // Too short: a session that has already ended is never made.
    for text in ["0", "0m", "0h", "0d", "000"] {
        let refused = run_at(&["session", text], now);
        assert_eq!(refused.code, 1, "{text}");
        assert_eq!(refused.out, "", "{text}: nothing on stdout");
        assert!(refused.err.contains("at least 1 minute"), "{text}: {}", refused.err);
        assert!(refused.err.contains("Nothing was made"), "{text}: {}", refused.err);
    }

    // Too long, or too big to count: refused by the core's range, never a crash.
    let thirty_two_nines = "9".repeat(32);
    let too_long = [
        "10081",
        "10081m",
        "169h",
        "8d",
        thirty_two_nines.as_str(),
        "99999999999999999h",
        "9223372036854775807",
        "9223372036854775807d",
        "9223372036854775808h",
    ];
    for text in too_long {
        let refused = run_at(&["session", text], now);
        assert_eq!(refused.code, 1, "{text}");
        assert_eq!(refused.out, "", "{text}: nothing on stdout");
        assert!(refused.err.contains("at most 10080 minutes (7 days)"), "{text}: {}", refused.err);
        assert!(refused.err.contains("Nothing was made"), "{text}: {}", refused.err);
    }

    // Text that is not a length: no sign, point, space, second unit, capital, other digits or words.
    let not_lengths = ["", "m", "h", "d", "1.5h", "-5", "+5", "1h30m", "2H", "5M", " 5", "5 ", "2 h", "٣٠", "２０", "0x10", "1e3", "5mm", "five"];
    for text in not_lengths {
        assert_eq!(read_length(text), None, "{text:?} is not a length");
    }

    // Counting saturates instead of wrapping or crashing.
    assert_eq!(count_of(&"9".repeat(40)), Some(i64::MAX));
    assert_eq!(count_of("9223372036854775808"), Some(i64::MAX));
    assert_eq!(count_of("12"), Some(12));
    assert_eq!(count_of(""), None);
    assert_eq!(count_of("1a"), None);
    assert_eq!(minutes_of(i64::MAX, 60), i64::MAX);
    assert_eq!(minutes_of(2, 60), 120);
    assert_eq!(minutes_of(7, 1_440), 10_080);
}
