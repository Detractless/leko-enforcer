//! `leko session 30` makes a session of that many minutes from the moment it is given and says what it
//! made (AC-001): the length, the end in UTC, and that nothing was saved or changed.

mod run_capture;

use leko_core::sessions::utc_moment::parse_utc_moment;
use run_capture::{expected_end, moment, run_at, MINUTE_MS};

const NOW: &str = "2026-11-05T10:00:00Z";
const ENDS_AT: &str = "ends at ";

/// The end the sentence names, in milliseconds.
fn end_in_sentence(sentence: &str) -> i64 {
    let at = sentence.find(ENDS_AT).unwrap_or_else(|| panic!("the sentence says when it ends: {sentence:?}")) + ENDS_AT.len();
    parse_utc_moment(&sentence[at..at + 20]).unwrap_or_else(|| panic!("a moment in {sentence:?}"))
}

#[test]
fn minutes_make_a_session_that_ends_that_long_after_the_given_moment() {
    let now = moment(NOW);

    // The sentence: length, end in UTC, and that nothing was saved; a part of a second rounds up.
    let made = run_at(&["session", "30"], now + 400);
    assert_eq!(made.code, 0, "{}", made.err);
    assert_eq!(made.err, "", "nothing on stderr when a session is made");
    assert_eq!(made.out.lines().count(), 1, "one line: {:?}", made.out);
    assert!(made.out.ends_with('\n'));
    assert!(made.out.contains("30 minutes"), "{}", made.out);
    assert!(made.out.contains("2026-11-05T10:30:01Z"), "10:00:00.400 plus 30 minutes ends 10:30:01: {}", made.out);
    assert!(made.out.contains("(UTC)"), "{}", made.out);
    assert!(made.out.contains("Nothing was saved or changed"), "{}", made.out);

    // Exactly on a whole second: no rounding.
    let exact = run_at(&["session", "30"], now);
    assert!(exact.out.contains("2026-11-05T10:30:00Z"), "{}", exact.out);
    let just_over = run_at(&["session", "30"], now + 1);
    assert!(just_over.out.contains("2026-11-05T10:30:01Z"), "{}", just_over.out);

    // One minute is not "1 minutes".
    let one = run_at(&["session", "1"], now);
    assert!(one.out.contains("session of 1 minute that"), "{}", one.out);
    assert!(!one.out.contains("1 minutes"), "{}", one.out);

    // The end is the moment handed in plus the length: other moments, other lengths, across a leap
    // day, a year end, and the epoch itself.
    let starts = [now, now + 1, now + 400, now + 999, moment("2028-02-29T23:30:00Z"), moment("2026-12-31T23:59:30Z"), 0, moment("2027-03-14T06:30:00Z") + 500];
    for start in starts {
        for minutes in [1_i64, 2, 59, 60, 61, 1_440, 10_080] {
            let text = minutes.to_string();
            let run = run_at(&["session", &text], start);
            assert_eq!(run.code, 0, "{minutes} minutes from {start}: {}", run.err);
            assert_eq!(end_in_sentence(&run.out), expected_end(start, minutes), "{minutes} minutes from {start}");
        }
    }

    // A 2-hour session across a clock change in New York ends exactly 7200000 ms later in UTC.
    for start in ["2027-03-14T06:30:00Z", "2026-11-01T05:30:00Z"] {
        let begin = moment(start);
        let run = run_at(&["session", "2h"], begin);
        assert_eq!(end_in_sentence(&run.out) - begin, 120 * MINUTE_MS, "from {start}");
    }
    assert!(run_at(&["session", "2h"], moment("2027-03-14T06:30:00Z")).out.contains("2027-03-14T08:30:00Z"));
}
