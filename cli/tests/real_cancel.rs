//! The real `leko` program, started as a process with the real clock and the zone from `TZ`, parks a
//! change, shows it in `status`, takes it back with `cancel`, and can park it again (AC-013). Results go
//! to stdout, problems to stderr, with the codes 0, 1 and 2.

mod delay_runs;
#[path = "../../core/tests/delay_sets/mod.rs"]
mod delay_sets;
mod run_capture;
mod scratch_folder;
#[path = "../../core/tests/zip_writer/mod.rs"]
mod zip_writer;

use delay_runs::{covering, one_line, records_of, snapshot, waiting_of, write_zip};
use delay_sets::wait;
use leko_cli::session::session_arguments::USAGE;
use leko_core::config_time::zone_time::ZoneTime;
use leko_core::sessions::utc_moment::parse_utc_moment;
use run_capture::Outcome;
use scratch_folder::Scratch;
use serde_json::{json, Value};
use std::path::Path;
use std::process::{Command, Stdio};

/// `leko ARGS...` as a process on a computer whose zone is `tz`.
fn leko(args: &[&str], tz: &str) -> Outcome {
    let output = Command::new(env!("CARGO_BIN_EXE_leko")).args(args).stdin(Stdio::null()).env("TZ", tz).output().expect("the program starts");
    Outcome {
        code: output.status.code().expect("the program ended by itself"),
        out: String::from_utf8(output.stdout).expect("stdout is text"),
        err: String::from_utf8(output.stderr).expect("stderr is text"),
    }
}

fn text(path: &Path) -> &str {
    path.to_str().expect("a text path")
}

fn done(run: &Outcome, what: &str) {
    assert_eq!((run.code, run.err.as_str()), (0, ""), "{what}: {}", run.out);
}

#[test]
fn the_real_program_parks_shows_cancels_and_starts_again() {
    let folder = Scratch::new("real-cancel");
    let file = write_zip(folder.path(), "s.zip", &[covering("slow", wait(24), &["slow.example.com"])], &[]);
    let name = text(&file);
    let zip_before = std::fs::read(&file).unwrap();

    // Park the change, on a computer in Tokyo.
    let parked = leko(&["switch-off", name, "--id", "slow", "--write"], "Asia/Tokyo");
    done(&parked, "park");
    assert!(parked.out.starts_with("Waiting: the block \"slow\" is not switched off yet."), "{}", parked.out);
    let first = records_of(&file);
    assert_eq!(first.len(), 1);
    let (requested, due) = (parse_utc_moment(first[0]["requestedAt"].as_str().unwrap()).unwrap(), parse_utc_moment(first[0]["dueAt"].as_str().unwrap()).unwrap());
    assert_eq!(due, requested + 24 * 3_600_000);
    let local = |zone: &str| format!("it may be switched off from {} ({zone}), held by:", ZoneTime::new(zone).unwrap().local_text(due));

    // Status lists it, for the app and for the site, in the zone of this computer or of --zone; the
    // JSON line gives the moment in UTC.
    for target in [["--app", "com.slow"], ["--site", "slow.example.com"]] {
        for (zone, flags) in [("Asia/Tokyo", vec![]), ("America/New_York", vec!["--zone", "America/New_York"])] {
            let run = leko(&[&["status"][..], &target, &flags, &[name]].concat(), "Asia/Tokyo");
            done(&run, &format!("{target:?} {zone}"));
            let line = format!("The block \"slow\" is not due yet: {}", local(zone));
            assert!(run.out.contains(&line) && run.out.ends_with("\nNothing was saved or changed.\n"), "{target:?} {zone}: {}", run.out);
        }
        let json_run = leko(&[&["status", "--json"][..], &target, &[name]].concat(), "Asia/Tokyo");
        done(&json_run, "json");
        let line: Value = serde_json::from_str(json_run.out.trim_end()).expect("one JSON line");
        let from = ZoneTime::new("UTC").unwrap().local_text(due).replace("+00:00", "Z");
        assert_eq!(line["waiting"], json!([{"block": "slow", "kind": "switch-off", "outcome": "not-yet", "from": from}]), "{target:?}");
    }
    let elsewhere = leko(&["status", "--app", "com.elsewhere", name], "Asia/Tokyo");
    done(&elsewhere, "another app");
    assert!(!elsewhere.out.contains("slow"), "{}", elsewhere.out);

    // A draft changes nothing; then the cancel takes the record out and never touches the zip.
    let waiting_before = std::fs::read(waiting_of(&file)).unwrap();
    let draft = leko(&["cancel", name, "--id", "slow"], "Asia/Tokyo");
    done(&draft, "draft");
    assert!(draft.out.starts_with("Draft: the waiting change for the block \"slow\" would be cancelled. Nothing was saved.\n"), "{}", draft.out);
    assert_eq!(std::fs::read(waiting_of(&file)).unwrap(), waiting_before, "a draft saves nothing");
    let cancelled = leko(&["cancel", name, "--id", "slow", "--write"], "Asia/Tokyo");
    done(&cancelled, "cancel");
    assert!(cancelled.out.starts_with("Cancelled: the waiting change for the block \"slow\" is taken back"), "{}", cancelled.out);
    assert_eq!(std::fs::read(&file).unwrap(), zip_before, "the settings zip was never touched");
    assert!(records_of(&file).is_empty());
    assert_eq!(folder.names(), ["s.zip", "s.zip.waiting.json"], "no temporary file is left");

    // It is gone from status and from settle, and a second cancel has nothing to take back.
    let after = leko(&["status", "--app", "com.slow", name], "Asia/Tokyo");
    done(&after, "status after");
    assert!(!after.out.contains("not due yet") && after.out.matches('\n').count() == 1, "{}", after.out);
    let listed = leko(&["settle", name], "Asia/Tokyo");
    done(&listed, "settle");
    assert_eq!(listed.out, "Nothing is waiting. Nothing was saved.\n");
    let again = leko(&["cancel", name, "--id", "slow", "--write"], "Asia/Tokyo");
    done(&again, "second cancel");
    assert_eq!(again.out, "Nothing is waiting for the block \"slow\". Nothing was saved.\n");

    // Asking again later starts from the new moment, not from the old record.
    std::thread::sleep(std::time::Duration::from_millis(1_100));
    done(&leko(&["switch-off", name, "--id", "slow", "--write"], "Asia/Tokyo"), "park again");
    let second = records_of(&file);
    assert_eq!(second.len(), 1);
    assert!(parse_utc_moment(second[0]["requestedAt"].as_str().unwrap()).unwrap() > requested, "a new, later request moment");
    assert_eq!(std::fs::read(&file).unwrap(), zip_before);

    // Problems go to stderr, with the code that says which kind they are.
    for args in [vec!["cancel", name], vec!["cancel", name, "--id"], vec!["cancel", name, "--id", "slow", "--zone", "UTC"], vec!["cancel"]] {
        let run = leko(&args, "UTC");
        assert_eq!((run.code, run.out.as_str()), (2, ""), "{args:?}: {}", run.err);
        assert!(run.err.ends_with(&format!("{USAGE}\n")), "{args:?}: {}", run.err);
    }
    std::fs::write(waiting_of(&file), b"not json").unwrap();
    let before_bad = snapshot(folder.path());
    for args in [vec!["cancel", name, "--id", "slow", "--write"], vec!["cancel", name, "--id", "slow"]] {
        let run = leko(&args, "UTC");
        assert_eq!((run.code, run.out.as_str()), (1, ""), "{args:?}: {}", run.err);
        assert!(one_line(&run.err).ends_with("It was left as it was. Nothing was saved.\n") && run.err.contains("is not valid JSON"), "{}", run.err);
    }
    assert_eq!(snapshot(folder.path()), before_bad, "a bad waiting file is left as it was");
    // Status still answers, with the problem on stderr only.
    let status = leko(&["status", "--app", "com.slow", name], "UTC");
    assert_eq!(status.code, 0, "{}", status.err);
    assert!(status.err.ends_with("The parked changes could not be listed.\n"), "{}", status.err);
    assert!(!status.out.contains("waiting file"), "{}", status.out);
}
