//! `leko voice` takes the words a speech-to-text heard. It makes exactly the session `leko session`
//! makes for the length it understood (AC-006), and it makes nothing, exit 2, when the words are
//! unclear or ask for the opposite (AC-007): a wrong guess would seal the user in for up to 7 days.

mod run_capture;

use leko_cli::session::voice_phrase::read_phrase;
use run_capture::{moment, run_at, Outcome};

const NOW: &str = "2026-11-05T10:00:00Z";

fn same(a: &Outcome, b: &Outcome, what: &str) {
    assert_eq!((a.code, &a.out, &a.err), (b.code, &b.out, &b.err), "{what}");
}

#[test]
fn spoken_phrases_make_the_same_session_as_the_typed_length() {
    let now = moment(NOW) + 400;
    let phrases = [
        ("start a 25 minute session", "25"),
        ("Start a 25-minute session!", "25"),
        ("START A 25 MINUTE SESSION", "25"),
        ("  start   a 25 minute   session  ", "25"),
        ("make a session for 2 hours", "2h"),
        ("begin a 90 minutes session", "90"),
        ("a 45 min session", "45"),
        ("a 45 mins session", "45"),
        ("a 3 hrs session", "3h"),
        ("3 hr session", "3h"),
        ("a session for an hour", "1h"),
        ("a session for one hour", "1h"),
        ("a session for a minute", "1"),
        ("a one day session", "1d"),
        ("a session for a day", "1d"),
        ("a session for 7 days", "7d"),
        ("a half hour session", "30"),
        ("a session for half an hour", "30"),
        ("half an hour session", "30"),
        ("Half Hour session.", "30"),
        ("session, 20 minutes", "20"),
        ("I'd like a 15 minute session please", "15"),
        ("please set up one session of 25 minutes", "25"),
        ("start a 1 hour session", "1h"),
        ("start a session for 30. minutes", "30"),
        ("a 25 minute session, thanks.", "25"),
        ("Démarre a 25 minute session s'il vous plaît", "25"),
    ];
    for (phrase, typed) in phrases {
        let heard = run_at(&["voice", phrase], now);
        let typed_run = run_at(&["session", typed], now);
        assert_eq!(heard.code, 0, "{phrase:?}: {}", heard.err);
        same(&heard, &typed_run, phrase);
        let heard_json = run_at(&["voice", "--json", phrase], now);
        same(&heard_json, &run_at(&["session", "--json", typed], now), phrase);
        assert_eq!(read_phrase(phrase).ok(), Some(read_typed(typed)), "{phrase:?}");
    }

    // Words may arrive as one argument or as several, and a name or --json may come first or last.
    let one = run_at(&["voice", "--json", "start a 25 minute session"], now);
    let several = run_at(&["voice", "start", "a", "25", "minute", "session", "--json"], now);
    same(&one, &several, "one argument or several");
    let named = run_at(&["voice", "--id", "focus", "--json", "start a 25 minute session"], now);
    same(&named, &run_at(&["session", "--id", "focus", "--json", "25"], now), "a name by voice");

    // Understood but refused: the length is outside 1 minute to 7 days; the core's sentence, exit 1.
    for phrase in ["a session for 8 days", "a 169 hour session", "a 10081 minute session", "a 99999999999999999999999999 minute session"] {
        let refused = run_at(&["voice", phrase], now);
        assert_eq!((refused.code, refused.out.as_str()), (1, ""), "{phrase:?}");
        assert!(refused.err.contains("at most 10080 minutes (7 days)") && refused.err.contains("Nothing was made"), "{phrase:?}: {}", refused.err);
    }
}

/// The minutes of a typed length, as the typed command reads them.
fn read_typed(typed: &str) -> i64 {
    leko_cli::session::session_length::read_length(typed).expect("a typed length")
}

#[test]
fn unclear_or_opposite_phrases_make_nothing() {
    let now = moment(NOW);
    let groups: [(&str, Vec<&str>); 14] = [
        ("no session word", vec!["25 minutes", "start a timer for 25 minutes", "sessions of 25 minutes", "block for 30 minutes"]),
        ("no length", vec!["start a session", "start a long session", "a session", "session session", "session for a while"]),
        (
            "two lengths",
            vec!["a 10 minute session or 20 minutes", "10 or 20 minutes session", "session 25 minutes 30 minutes", "a 25 minute session for 25 minutes", "a half hour or 1 hour session"],
        ),
        ("an unknown unit", vec!["a 30 second session", "a 3 week session", "a session for 2 months", "a 1 year session", "a session of a fortnight", "a 30 secs session"]),
        ("a number in words", vec!["start a twenty minute session", "a two hour session", "session for five minutes", "a couple of hours session", "a hundred minute session", "a few minutes session"]),
        ("zero", vec!["a 0 minute session", "a zero minute session", "a 00 hour session", "a session for 0 days"]),
        (
            "a minus, a range or a decimal",
            vec![
                "a -5 minute session",
                "a session for -5 minutes",
                "a 1.5 hour session",
                "a 1,5 hour session",
                "a 2-3 hour session",
                "a 1,000 minute session",
                "a \u{2212}5 minute session",
                "a 2\u{2013}3 hour session",
                "a \u{2014}5 minute session",
                "a .5 hour session",
                "a session for ,5 hours",
                ".5 hours session",
                "start a session,25 minutes",
                "a session.25 minutes",
            ],
        ),
        ("half in another form", vec!["an hour and a half session", "a session for a half", "a half day session", "a quarter hour session", "half a session"]),
        (
            "cancelling words",
            vec![
                "cancel my 30 minute session",
                "stop the 30 minute session",
                "end the 30 minute session",
                "the 30 minute session ends",
                "remove my 30 minute session",
                "delete the 30 minute session",
                "undo that 30 minute session",
                "unlock my 30 minute session",
                "shorten the 30 minute session",
                "skip the 30 minute session",
                "disable the 30 minute session",
                "quit the 30 minute session",
                "abort the 30 minute session",
                "canceling the 30 minute session",
                "CANCELLED: 30 minute session",
                "stopping my 30 minute session",
            ],
        ),
        (
            "denying words",
            vec![
                "no 30 minute session",
                "I do not want a 30 minute session",
                "don't start a 10 minute session",
                "Dont start a 10 minute session",
                "don\u{2019}t start a 10 minute session",
                "never start a 10 minute session",
                "a 30 minute session without asking",
                "I can't do a 30 minute session",
                "I won't do a 30 minute session",
            ],
        ),
        (
            "a number with no unit, or a unit with no number",
            vec![
                "start a session at 5 pm for 30 minutes",
                "start a 30 minute session at 5pm",
                "start session 25",
                "25m session",
                "a minutes session",
                "a session in 25",
                "start a session for 25 o'clock",
                "a 1\u{ff12} minute session",
                "a 2\u{663} minutes session",
                "a 25 minute session hours",
                "hours a 25 minute session",
                "a session for 2\u{bd} hours",
            ],
        ),
        ("no words at all", vec!["", "   ", "!!!", "...", "\u{663}\u{660} minutes session", "\u{ff12}\u{ff10} minute session", "\n\t"]),
        ("a plural unit after a, an or one (a misheard number)", vec!["a hours session", "an hours session", "one days session", "a mins session", "a hrs session", "session for a minutes"]),
        ("another language", vec!["empieza una sesion de 25 minutos", "démarre une session de vingt minutes"]),
    ];
    for (why, phrases) in &groups {
        for phrase in phrases {
            let run = run_at(&["voice", phrase], now);
            assert_eq!(run.code, 2, "{why}: {phrase:?} made something: {}", run.out);
            assert_eq!(run.out, "", "{why}: {phrase:?}: nothing on stdout");
            assert!(run.err.starts_with("Nothing was made: "), "{why}: {phrase:?}: {}", run.err);
            assert!(run.err.contains("Heard: "), "{why}: {phrase:?}: it says what it heard: {}", run.err);
            assert!(run.err.contains("\"start a 25 minute session\""), "{why}: {phrase:?}: it names a form it takes: {}", run.err);
            assert!(read_phrase(phrase).is_err(), "{why}: {phrase:?}");
            assert_eq!(run_at(&["voice", "--json", phrase], now).out, "", "{why}: {phrase:?}: no JSON either");
        }
    }

    // Each reason is the real one, so the user can fix the words.
    let reason = |phrase: &str| read_phrase(phrase).expect_err(phrase);
    assert!(reason("cancel my 30 minute session").contains("\"cancel\""));
    assert!(reason("a 10 minute session or 20 minutes").contains("two lengths"));
    assert!(reason("a twenty minute session").contains("twenty"));
    assert!(reason("a 30 second session").contains("second"));
    assert!(reason("an hour and a half session").contains("half"));
    assert!(reason("a start").contains("session"));
    assert!(reason("a session").contains("no length"));
    assert!(reason("a 1.5 hour session").contains("decimal"));
    assert!(reason("a -5 minute session").contains("minus"));
    assert!(reason("two 5 minute session").contains("\"two\""));
    assert!(reason("25 minutes please").contains("\"session\""));
    assert!(reason("a hours session").contains("say it in digits"));
    assert!(reason("").contains("no words"));
    assert!(reason("a 30 seconds session").contains("seconds"));
    assert!(reason("a .5 hour session").contains("decimal"));
    assert!(reason("a 25 minute session hours").contains("no number before"));

    // The length limit: 500 characters are read, 501 are refused, and a huge text is refused fast
    // and not echoed whole.
    let padded = |length: usize| format!("{:<length$}", "start a 25 minute session");
    assert_eq!(padded(500).chars().count(), 500);
    assert_eq!(run_at(&["voice", &padded(500)], now).code, 0, "500 characters are read");
    let over = run_at(&["voice", &padded(501)], now);
    assert_eq!((over.code, over.out.as_str()), (2, ""), "501 characters are refused");
    assert!(over.err.contains("500 characters"), "{}", over.err);
    // What was heard is echoed cut to 60 characters, with dots only when it was cut.
    let sixty: String = "abcdefghij".repeat(6);
    let exact = run_at(&["voice", &sixty], now);
    assert!(exact.err.contains(&format!("Heard: \"{sixty}\". Say")), "{}", exact.err);
    let cut = run_at(&["voice", &format!("{sixty}k")], now);
    assert!(cut.err.contains(&format!("Heard: \"{sixty}...\". Say")), "{}", cut.err);
    assert!(!cut.err.contains(&format!("{sixty}k")), "{}", cut.err);

    // Characters are counted, not bytes: 500 two-byte characters are read, 501 are not.
    let accents = |length: usize| format!("start a 25 minute session {}", "é".repeat(length - 26));
    assert!(accents(500).len() > 500 && accents(500).chars().count() == 500);
    assert_eq!(run_at(&["voice", &accents(500)], now).code, 0, "500 characters of two bytes are read");
    assert_eq!(run_at(&["voice", &accents(501)], now).code, 2);
    let huge = "start a 25 minute session ".repeat(40_000);
    let refused = run_at(&["voice", &huge], now);
    assert_eq!((refused.code, refused.out.as_str()), (2, ""));
    assert!(refused.err.len() < 800, "the phrase is not echoed whole: {} bytes", refused.err.len());
    let noise = "a".repeat(1_000_000);
    assert_eq!(run_at(&["voice", &noise], now).code, 2);

    // A phrase with terminal control characters is written out, never sent raw to the terminal.
    let escape = run_at(&["voice", "\u{1b}[31m start a session"], now);
    assert_eq!(escape.code, 2);
    assert!(!escape.err.contains('\u{1b}'), "{:?}", escape.err);
}

/// A phrase that is understood, to which one more word is added: the word alone must change it.
const GOOD: &str = "a 25 minute session";

#[test]
fn each_word_that_cancels_or_names_another_length_refuses_the_phrase() {
    assert_eq!(read_phrase(GOOD), Ok(25), "the plain phrase is understood");

    let denying = ["no", "not", "dont", "cant", "wont", "never", "without", "end", "ends", "ended", "ending", "quit", "quits", "quitting", "undo"];
    let stems = ["cancel", "stop", "remov", "delet", "unlock", "shorten", "disabl", "abort", "skip"];
    for word in denying {
        assert!(read_phrase(&format!("{GOOD} {word}")).is_err(), "{word:?} refuses the phrase");
    }
    for stem in stems {
        for ending in ["", "ed", "ing", "xyz"] {
            assert!(read_phrase(&format!("{GOOD} {stem}{ending}")).is_err(), "{stem}{ending} refuses the phrase");
        }
    }

    let other_units = [
        "second", "seconds", "sec", "secs", "millisecond", "milliseconds", "week", "weeks", "fortnight", "fortnights", "month", "months", "year", "years", "decade",
        "decades",
    ];
    for unit in other_units {
        assert!(read_phrase(&format!("{GOOD} {unit}")).is_err(), "{unit:?} alone refuses the phrase");
        assert!(read_phrase(&format!("{GOOD} for a {unit}")).is_err(), "a {unit:?} refuses the phrase");
        assert!(read_phrase(&format!("session for 25 {unit}")).is_err(), "25 {unit:?} is no length");
        assert!(read_phrase(&format!("{GOOD} for one {unit}")).is_err(), "one {unit:?} refuses the phrase");
    }

    let number_words = [
        "zero", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten", "eleven", "twelve", "thirteen", "fourteen", "fifteen", "sixteen", "seventeen",
        "eighteen", "nineteen", "twenty", "thirty", "forty", "fifty", "sixty", "seventy", "eighty", "ninety", "hundred", "thousand", "million", "couple", "few",
        "several", "dozen", "quarter",
    ];
    for word in number_words {
        assert!(read_phrase(&format!("{GOOD} {word}")).is_err(), "{word:?} refuses the phrase");
        assert!(read_phrase(&format!("{word} {GOOD}")).is_err(), "{word:?} first refuses the phrase");
    }

    // Every unit is read, in its singular and plural, short and long forms.
    let units: [(&str, i64); 10] = [
        ("minute", 1),
        ("minutes", 1),
        ("min", 1),
        ("mins", 1),
        ("hour", 60),
        ("hours", 60),
        ("hr", 60),
        ("hrs", 60),
        ("day", 1_440),
        ("days", 1_440),
    ];
    for (unit, size) in units {
        assert_eq!(read_phrase(&format!("a session of 3 {unit}")), Ok(3 * size), "3 {unit}");
        assert_eq!(read_phrase(&format!("a session of 12 {unit}")), Ok(12 * size), "12 {unit}");
    }
    for (unit, size) in [("minute", 1), ("min", 1), ("hour", 60), ("hr", 60), ("day", 1_440)] {
        for one in ["a", "an", "one"] {
            assert_eq!(read_phrase(&format!("a session of {one} {unit}")), Ok(size), "{one} {unit}");
        }
    }
}
