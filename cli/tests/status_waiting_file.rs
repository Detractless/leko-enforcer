//! A waiting file that cannot be read does not take `leko status` down (AC-012): the answer goes to
//! stdout as it always was (exit 0), the one-sentence problem goes to stderr, the JSON `waiting` is
//! `null`, and the file is not touched. A file that is not there is not a problem and says nothing.

mod parked_change_support;
use parked_change_support::{delay_runs, delay_sets, run_capture, scratch_folder, zip_writer};

use delay_runs::{one_line, snapshot, status_blocks, waiting_of, write_zip, NOW};
use leko_cli::session::session_report::shown;
use run_capture::{json_of, status};
use scratch_folder::Scratch;
use serde_json::{json, Value};

const GOOD: &str = r#"{"kind":"switch-off","block":"a","requestedAt":"2026-11-02T12:00:00Z","dueAt":"2026-11-03T12:00:00Z"}"#;
const NY: [&str; 2] = ["--zone", "America/New_York"];

fn bad_files() -> Vec<(&'static str, Vec<u8>, &'static str)> {
    let mut big = br#"{"format":1,"changes":[]}"#.to_vec();
    big.resize(1_048_577, b' ');
    vec![
        ("empty", Vec::new(), "is empty"),
        ("not JSON", b"hello".to_vec(), "is not valid JSON"),
        ("not text", vec![0xFF, 0xFE], "is not valid JSON"),
        ("a newer format", format!(r#"{{"format":4,"changes":[{GOOD}]}}"#).into_bytes(), "is format 4, which is newer"),
        ("a bad record", format!(r#"{{"format":1,"changes":[{GOOD},{{"kind":"x","block":"b"}}]}}"#).into_bytes(), "was refused: change 2"),
        ("over 1 MB", big, "larger than 1 MB"),
    ]
}

#[test]
fn status_still_answers_when_the_waiting_file_is_bad_and_says_so_on_stderr() {
    let plain_folder = Scratch::new("status-file-plain");
    let plain = write_zip(plain_folder.path(), "s.zip", &status_blocks(), &[]);
    let site = ["--site", "example.com"];
    let sentence_flags = [&site[..], &NY[..]].concat();
    let json_flags = [&["--json"][..], &site[..], &NY[..]].concat();
    let old_sentence = status(&plain, &sentence_flags, NOW, None);
    let old_json = json_of(&status(&plain, &json_flags, NOW, None));
    assert_eq!((old_sentence.code, old_sentence.err.as_str()), (0, ""));

    for (what, bytes, reason) in bad_files() {
        let folder = Scratch::new("status-file");
        let file = write_zip(folder.path(), "s.zip", &status_blocks(), &[]);
        std::fs::write(waiting_of(&file), &bytes).unwrap();
        let before = snapshot(folder.path());
        let start = format!("The waiting file {}", shown(&waiting_of(&file).to_string_lossy()));

        // The answer is the old one on stdout, exit 0; the problem is one line on stderr only.
        let run = status(&file, &sentence_flags, NOW, None);
        assert_eq!((run.code, run.out.as_str()), (0, old_sentence.out.as_str()), "{what}: {}", run.err);
        let problem = one_line(&run.err);
        assert!(problem.starts_with(&start) && problem.contains(reason), "{what}: {problem}");
        assert!(problem.ends_with("It was left as it was. The parked changes could not be listed.\n"), "{what}: {problem}");
        assert!(!run.out.contains("waiting file"), "{what}: the problem is never part of the answer");

        // The JSON line says "unreadable" with null, not "none" with an empty list.
        let run = status(&file, &json_flags, NOW, None);
        assert_eq!(run.code, 0, "{what}: {}", run.err);
        assert_eq!(one_line(&run.err), problem, "{what}");
        let mut line = json_of(&run);
        assert_eq!(line["waiting"], Value::Null, "{what}");
        line["waiting"] = json!([]);
        assert_eq!(line, old_json, "{what}: the rest of the line is the same");
        assert_eq!(snapshot(folder.path()), before, "{what}: the file was not touched");
    }

    // A folder in place of the file is a problem too; a missing file is not.
    let folder = Scratch::new("status-file-dir");
    let file = write_zip(folder.path(), "s.zip", &status_blocks(), &[]);
    std::fs::create_dir(waiting_of(&file)).unwrap();
    let run = status(&file, &sentence_flags, NOW, None);
    assert_eq!((run.code, run.out.as_str()), (0, old_sentence.out.as_str()));
    assert!(one_line(&run.err).contains("is not a file"), "{}", run.err);
    std::fs::remove_dir(waiting_of(&file)).unwrap();
    for flags in [&sentence_flags, &json_flags] {
        let run = status(&file, flags, NOW, None);
        assert_eq!((run.code, run.err.as_str()), (0, ""), "a file that is not there says nothing");
    }
    assert_eq!(folder.names(), ["s.zip"], "and none is made");
}
