//! `leko status --json` always carries a `"waiting"` list (AC-011): one object per applying change with
//! the block, the kind, the outcome word and the UTC moment it may land (`null` when there is none);
//! an empty list when nothing applies, so a script can tell "none" from an older program; the moments
//! are in UTC whatever zone is chosen.

mod delay_runs;
#[path = "../../core/tests/delay_sets/mod.rs"]
mod delay_sets;
mod run_capture;
mod scratch_folder;
#[path = "../../core/tests/zip_writer/mod.rs"]
mod zip_writer;

use delay_runs::{covering, record, status_blocks, status_records, write_waiting, write_zip, NOW};
use delay_sets::{sealed_until, wait};
use run_capture::{json_of, status};
use scratch_folder::Scratch;
use serde_json::{json, Value};

fn line_for(file: &std::path::Path, flags: &[&str], zone: &str) -> Value {
    let run = status(file, &[&["--json"][..], flags, &["--zone", zone]].concat(), NOW, None);
    assert_eq!((run.code, run.err.as_str()), (0, ""), "{flags:?}: {}", run.err);
    json_of(&run)
}

fn change(block: &str, outcome: &str, from: Value) -> Value {
    json!({"block": block, "kind": "switch-off", "outcome": outcome, "from": from})
}

#[test]
fn the_json_line_always_has_a_waiting_list_in_utc() {
    let folder = Scratch::new("status-json");
    let file = write_zip(folder.path(), "s.zip", &status_blocks(), &[]);

    // No waiting file: the key is there and the list is empty.
    let line = line_for(&file, &["--site", "example.com"], "America/New_York");
    assert!(line.as_object().expect("an object").contains_key("waiting"));
    assert_eq!(line["waiting"], json!([]));

    write_waiting(&file, &status_records());
    // Two blocks list the site: their objects in file order, with the UTC moment, in any zone.
    let both = json!([change("s", "not-yet", json!("2026-11-04T13:00:00Z")), change("a", "not-yet", json!("2026-11-03T12:00:00Z"))]);
    for zone in ["America/New_York", "Asia/Tokyo", "UTC"] {
        assert_eq!(line_for(&file, &["--site", "example.com"], zone)["waiting"], both, "{zone}: UTC, not local time");
    }
    // The rest of the answer is what it always was.
    let bare = line_for(&write_zip(Scratch::new("status-json-bare").path(), "s.zip", &status_blocks(), &[]), &["--site", "example.com"], "UTC");
    let mut without = line_for(&file, &["--site", "example.com"], "UTC");
    without["waiting"] = json!([]);
    assert_eq!(without, bare);

    // The outcome words: due, a key needed, already off. A due change has no moment to wait for.
    assert_eq!(line_for(&file, &["--app", "com.c"], "UTC")["waiting"], json!([change("c", "due", Value::Null)]));
    assert_eq!(line_for(&file, &["--site", "keyed.net"], "UTC")["waiting"], json!([change("k", "needs-partner-key", Value::Null)]));
    assert_eq!(line_for(&file, &["--app", "com.o"], "UTC")["waiting"], json!([change("o", "already-off", Value::Null)]));
    // A change nobody can land by themselves.
    let never_folder = Scratch::new("status-json-never");
    let never = write_zip(never_folder.path(), "s.zip", &[covering("never", sealed_until("9999-12-31"), &["never.example"])], &[]);
    write_waiting(&never, &[record("never", "2026-11-02T12:00:00Z", "2026-11-02T13:00:00Z")]);
    assert_eq!(line_for(&never, &["--site", "never.example"], "UTC")["waiting"], json!([change("never", "cannot-land", Value::Null)]));

    // Nothing applies (another target, a block that is gone, an app named like a site): an empty list.
    for flags in [&["--app", "com.zzz"][..], &["--site", "nowhere.net"], &["--app", "example.com"], &["--app", "com.gone"]] {
        assert_eq!(line_for(&file, flags, "UTC")["waiting"], json!([]), "{flags:?}");
    }
    // Every object has exactly the four keys.
    for object in both.as_array().unwrap() {
        assert_eq!(object.as_object().unwrap().len(), 4);
    }
    let _ = wait(1);
}
