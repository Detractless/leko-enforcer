//! The real `leko` program, started as a process with the real clock, switches a block off, sees in
//! `status` that its app is open, switches it on again and sees it closed (AC-012); a block with a parked
//! switch-off is refused until `cancel --write` clears the way. Results go to stdout, problems to stderr,
//! with the codes 0 (done, or nothing to do), 1 (refused) and 2 (not understood).

mod delay_runs;
#[path = "../../core/tests/delay_sets/mod.rs"]
mod delay_sets;
mod run_capture;
mod scratch_folder;
#[path = "../../core/tests/zip_writer/mod.rs"]
mod zip_writer;

use delay_runs::{covering, one_line, record, records_of, snapshot, write_waiting, write_zip};
use delay_sets::wait;
use leko_cli::session::session_arguments::USAGE;
use run_capture::Outcome;
use scratch_folder::Scratch;
use serde_json::{json, Value};
use std::path::Path;
use std::process::{Command, Stdio};

/// `leko ARGS...` as a process on a computer whose zone is UTC.
fn leko(args: &[&str]) -> Outcome {
    let output = Command::new(env!("CARGO_BIN_EXE_leko")).args(args).stdin(Stdio::null()).env("TZ", "UTC").output().expect("the program starts");
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

/// Whether `status` says the app `com.ID` is closed now.
fn closed(file: &Path, id: &str) -> bool {
    let run = leko(&["status", "--json", "--app", &format!("com.{id}"), text(file)]);
    done(&run, &format!("status of {id}"));
    let line: Value = serde_json::from_str(run.out.trim_end()).expect("one JSON line");
    line["closed"].as_bool().expect("a yes or no")
}

#[test]
fn the_real_program_switches_off_and_on_again_and_status_follows() {
    let folder = Scratch::new("real-switch-on");
    let held = {
        let mut block = covering("held", wait(24), &[]);
        block["on"] = json!(false);
        block
    };
    let file = write_zip(folder.path(), "s.zip", &[covering("free", json!(null), &[]), held], &[]);
    let name = text(&file);

    // Off at once, open in status; a draft of the switch-on changes nothing; then it is closed again.
    assert!(closed(&file, "free"), "the block closes its app to begin with");
    let off = leko(&["switch-off", name, "--id", "free", "--write"]);
    done(&off, "switch-off");
    assert!(off.out.starts_with("Saved: the block \"free\" is switched off in "), "{}", off.out);
    assert!(!closed(&file, "free"), "switched off, the app is open");
    let after_off = snapshot(folder.path());
    let draft = leko(&["switch-on", name, "--id", "free"]);
    done(&draft, "draft");
    assert!(draft.out.starts_with("Draft: the block \"free\" would be switched on now. Nothing was saved.\n"), "{}", draft.out);
    assert_eq!(snapshot(folder.path()), after_off, "a draft saves nothing");
    assert!(!closed(&file, "free"), "still open after a draft");
    let on = leko(&["switch-on", name, "--id", "free", "--write"]);
    done(&on, "switch-on");
    assert!(on.out.starts_with("Saved: the block \"free\" is switched on in "), "{}", on.out);
    assert!(closed(&file, "free"), "switched on, the app is closed again");
    assert_eq!(folder.names(), ["s.zip"], "no temporary file and no waiting file");

    // A second one has nothing to do, and says so.
    let again = leko(&["switch-on", name, "--id", "free", "--write"]);
    done(&again, "again");
    assert_eq!(again.out, "The block \"free\" is already on. Nothing was saved.\n");

    // A parked switch-off stops the switch-on until it is cancelled.
    write_waiting(&file, &[record("held", "2026-11-02T14:00:00Z", "2026-11-03T14:00:00Z")]);
    assert!(!closed(&file, "held"), "the block is off");
    let before_stop = snapshot(folder.path());
    let stopped = leko(&["switch-on", name, "--id", "held", "--write"]);
    assert_eq!((stopped.code, stopped.out.as_str()), (1, ""), "{}", stopped.err);
    assert!(one_line(&stopped.err).contains("has a switch-off waiting, which would undo a switch-on. Cancel it first: leko cancel ") && stopped.err.ends_with(" --id held --write. Nothing was saved.\n"), "{}", stopped.err);
    assert_eq!(snapshot(folder.path()), before_stop, "a refusal saves nothing");
    done(&leko(&["cancel", name, "--id", "held", "--write"]), "cancel");
    assert!(records_of(&file).is_empty());
    let through = leko(&["switch-on", name, "--id", "held", "--write"]);
    done(&through, "switch-on after the cancel");
    assert!(through.out.starts_with("Saved: the block \"held\" is switched on in "), "{}", through.out);
    assert!(closed(&file, "held"), "switched on, its app is closed");

    // Problems go to stderr, with the code that says which kind they are.
    for args in [vec!["switch-on", name], vec!["switch-on", name, "--id"], vec!["switch-on", name, "--id", "free", "--zone", "UTC"], vec!["switch-on"]] {
        let run = leko(&args);
        assert_eq!((run.code, run.out.as_str()), (2, ""), "{args:?}: {}", run.err);
        assert!(run.err.ends_with(&format!("{USAGE}\n")), "{args:?}: {}", run.err);
    }
    let before_missing = snapshot(folder.path());
    let missing = leko(&["switch-on", name, "--id", "nope", "--write"]);
    assert_eq!((missing.code, missing.out.as_str()), (1, ""), "{}", missing.err);
    assert!(one_line(&missing.err).starts_with("There is no block \"nope\" in ") && missing.err.ends_with(". Nothing was saved.\n"), "{}", missing.err);
    assert_eq!(snapshot(folder.path()), before_missing);
}
