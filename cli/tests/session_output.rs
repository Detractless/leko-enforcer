//! With `--json` stdout is exactly one line holding the rule and the seal the core's session maker
//! returns for the same moment, length and name (AC-005); without it stdout is the sentence only.

mod run_capture;

use leko_core::sessions::timed_session::make_timed_session;
use run_capture::{ends_in_json, expected_end, json_of, moment, run_at};
use serde_json::{json, Value};

#[test]
fn json_is_one_line_holding_the_rule_and_the_seal_the_core_makes() {
    let starts = [moment("2026-11-05T10:00:00Z"), moment("2026-11-05T10:00:00Z") + 400, moment("2028-02-29T23:59:59Z") + 999, 0];
    let ways: [(&str, Vec<&str>); 4] = [
        ("flag before the length", vec!["session", "--json", "{m}"]),
        ("flag after the length", vec!["session", "{m}", "--json"]),
        ("flag before and a name", vec!["session", "--json", "--id", "focus", "{m}"]),
        ("flag after and a name", vec!["session", "{m}", "--id", "focus", "--json"]),
    ];
    for start in starts {
        for minutes in [1_i64, 45, 120, 10_080] {
            for (way, template) in &ways {
                let text = minutes.to_string();
                let args: Vec<&str> = template.iter().map(|part| if *part == "{m}" { text.as_str() } else { *part }).collect();
                let run = run_at(&args, start);
                assert_eq!(run.code, 0, "{way}: {}", run.err);
                assert_eq!(run.err, "", "{way}: nothing on stderr");
                let id = if args.contains(&"focus") { "focus" } else { "session" };

                // Exactly one line, and nothing before or after it.
                assert!(run.out.ends_with('\n') && !run.out.ends_with("\n\n"), "{way}: {:?}", run.out);
                assert_eq!(run.out.lines().count(), 1, "{way}: {:?}", run.out);

                // The very rule and seal the core makes, and nothing else beside them.
                let made = make_timed_session(start, minutes, id).expect("the core makes it");
                let printed = json_of(&run);
                assert_eq!(printed, json!({"rule": made.rule, "lock": made.lock}), "{way}: {minutes} minutes from {start}");
                let keys: Vec<&String> = printed.as_object().expect("an object").keys().collect();
                assert_eq!(keys.len(), 2, "{way}: only rule and lock: {keys:?}");

                // Both end at the same moment, and it is the moment asked for.
                assert_eq!(printed["rule"]["endsAt"], printed["lock"]["at"], "{way}");
                assert_eq!(ends_in_json(&run), expected_end(start, minutes), "{way}");
                assert_eq!(ends_in_json(&run), made.ends_ms, "{way}");
            }
        }
    }

    // Without --json: the sentence form only, never JSON.
    let now = moment("2026-11-05T10:00:00Z");
    let sentence = run_at(&["session", "30"], now);
    assert_eq!(sentence.code, 0);
    assert!(sentence.out.contains("Nothing was saved or changed"), "{}", sentence.out);
    assert!(!sentence.out.contains('{') && !sentence.out.contains("sealedAt"), "{}", sentence.out);
    assert!(serde_json::from_str::<Value>(sentence.out.trim_end()).is_err(), "the sentence is not JSON");

    // With --json: no sentence beside the JSON.
    let json_run = run_at(&["session", "--json", "30"], now);
    assert!(!json_run.out.contains("Nothing was saved"), "{}", json_run.out);
    assert!(json_run.out.starts_with('{'), "{}", json_run.out);
}
