//! `leko settle` judges each block as it is now, not as it was when the change was parked (AC-014): a
//! lock tightened since pushes the moment later, a hand-edited earlier moment lands nothing early, the
//! second it is due is the second it lands, a block that is gone or already off has its record cleared,
//! and a partner key (or locks that leave no shared minute) stops the landing and keeps the record.

mod parked_change_support;
use parked_change_support::{delay_runs, delay_sets, run_capture, scratch_folder, zip_writer};

use delay_runs::{assert_switched_off, block_of, record, records_of, settle, settle_with, snapshot, waiting_of, write_waiting, write_zip, FailAt, NOW};
use delay_sets::{change_hours, locked_block, off_block, routine_holding, wait, with_partner_key};
use leko_cli::session::session_report::{quoted, shown};
use scratch_folder::Scratch;
use serde_json::Value;

/// A folder with one block `id` under `lock` and one record for it, and the file.
fn one_block(id: &str, lock: Value, requested: &str, due: &str) -> (Scratch, std::path::PathBuf) {
    let folder = Scratch::new("recheck-one");
    let file = write_zip(folder.path(), "s.zip", &[locked_block(id, lock)], &[]);
    write_waiting(&file, &[record(id, requested, due)]);
    (folder, file)
}

fn not_due(id: &str, local: &str, because: &str) -> String {
    format!("The block \"{id}\" is not due yet: it may be switched off from {local} (America/New_York), held by: {because}.\nNothing is due yet. Nothing was saved.\n")
}

#[test]
fn settle_judges_the_block_as_it_is_now_and_never_lands_early() {
    // The moment is the later of the saved one and the one the present locks give. Each case: the lock
    // now, the record, the moment shown, why, and the UTC moment it is due.
    let cases = [
        ("a lock tightened since (24 h to 72 h)", wait(72), "2026-11-02T10:00:00Z", "2026-11-03T10:00:00Z", "2026-11-05T05:00:00-05:00", "the block's own lock (a wait of 72 hours)", "2026-11-05T10:00:00Z"),
        ("a due moment edited to the request moment", wait(24), "2026-11-02T10:00:00Z", "2026-11-02T10:00:00Z", "2026-11-03T05:00:00-05:00", "the block's own lock (a wait of 24 hours)", "2026-11-03T10:00:00Z"),
        ("a lock loosened since (the saved moment is later)", wait(1), "2026-11-02T10:00:00Z", "2026-11-04T10:00:00Z", "2026-11-04T05:00:00-05:00", "the moment saved when the change was asked for", "2026-11-04T10:00:00Z"),
    ];
    for (what, lock, requested, saved_due, local, because, due) in cases {
        let (folder, file) = one_block("t", lock, requested, saved_due);
        let before = snapshot(folder.path());
        let second_before = previous_second(due);
        for now in [NOW, second_before.as_str()] {
            for flags in [&[][..], &["--write"]] {
                let run = settle(&file, flags, now);
                assert_eq!((run.code, run.err.as_str()), (0, ""), "{what} at {now}");
                assert_eq!(run.out, not_due("t", local, because), "{what} at {now} {flags:?}");
                assert_eq!(snapshot(folder.path()), before, "{what} at {now} {flags:?}: nothing landed early, nothing was rewritten");
            }
        }
        // The second it is due, it lands.
        let old = block_of(&file, "t");
        let run = settle(&file, &["--write"], due);
        assert_eq!((run.code, run.err.as_str()), (0, ""), "{what} at {due}");
        assert!(run.out.starts_with("The block \"t\" is switched off.\nSaved: 1 switched off in "), "{what}: {}", run.out);
        assert_switched_off(&old, &block_of(&file, "t"));
        assert!(records_of(&file).is_empty(), "{what}: the record is gone once it landed");
    }

    // A mixed list: one lands, two are cleared, three are stopped or not due and kept as they were.
    let folder = Scratch::new("recheck-mixed");
    let blocks = [
        locked_block("u", Value::Null),
        off_block("off"),
        locked_block("k", with_partner_key(wait(1))),
        locked_block("rk", Value::Null),
        locked_block("never", change_hours("08:00", "09:00", 0)),
        locked_block("w", wait(24)),
    ];
    let routines = [routine_holding("kr", &["rk"], with_partner_key(wait(1))), routine_holding("nr", &["never"], change_hours("20:00", "21:00", 0))];
    let file = write_zip(folder.path(), "s.zip", &blocks, &routines);
    let (earlier, later) = ("2026-11-02T10:00:00Z", "2026-11-02T11:00:00Z");
    let kept = [record("k", earlier, later), record("rk", earlier, later), record("never", earlier, later), record("w", "2026-11-02T14:00:00Z", "2026-11-03T14:00:00Z")];
    let all = [record("u", earlier, later), record("gone", earlier, later), record("off", earlier, later), kept[0].clone(), kept[1].clone(), kept[2].clone(), kept[3].clone()];
    write_waiting(&file, &all);
    let tail = |done: bool| {
        let first = if done { "The block \"u\" is switched off." } else { "The block \"u\" is due and would be switched off." };
        format!(
            "{first}\nThe block \"gone\" is no longer in the settings, so its waiting change is cleared.\nThe block \"off\" is already off, so its waiting change is cleared.\n\
             The block \"k\" now has a lock that needs a partner's key, so it was not switched off; its waiting change is kept.\n\
             The block \"rk\" now has a lock that needs a partner's key, so it was not switched off; its waiting change is kept.\n\
             The locks on the block \"never\" never leave a moment to change it, so its waiting change is kept.\n\
             The block \"w\" is not due yet: it may be switched off from 2026-11-03T09:00:00-05:00 (America/New_York), held by: the block's own lock (a wait of 24 hours).\n"
        )
    };
    let (before, old_u) = (snapshot(folder.path()), block_of(&file, "u"));
    let command = format!("leko settle {}", quoted(&file.to_string_lossy()));
    let draft = settle(&file, &[], NOW);
    assert_eq!((draft.code, draft.err.as_str()), (0, ""));
    assert_eq!(draft.out, format!("{}Draft: nothing was saved. To save this, run: {command} --write\n", tail(false)));
    assert_eq!(snapshot(folder.path()), before, "the draft saved nothing");

    let run = settle(&file, &["--write"], NOW);
    assert_eq!((run.code, run.err.as_str()), (0, ""));
    assert_eq!(run.out, format!("{}Saved: 1 switched off in {}, 2 cleared and 4 still waiting in {}.\n", tail(true), shown(&file.to_string_lossy()), shown(&waiting_of(&file).to_string_lossy())));
    assert_switched_off(&old_u, &block_of(&file, "u"));
    for id in ["k", "rk", "never", "w"] {
        assert_ne!(block_of(&file, id)["on"], Value::Bool(false), "{id} stayed on");
    }
    assert_eq!(records_of(&file), kept, "the stopped and not-due records are kept exactly, in order");

    // Only clearing: the zip is not rewritten at all, and only the waiting file is saved.
    let folder = Scratch::new("recheck-clear");
    let file = write_zip(folder.path(), "s.zip", &[off_block("off")], &[]);
    write_waiting(&file, &[record("gone", earlier, later), record("off", earlier, later)]);
    let zip_before = std::fs::read(&file).unwrap();
    let mut steps = FailAt::recording();
    let run = settle_with(&file, &["--write"], NOW, &mut steps);
    assert_eq!((run.code, run.err.as_str()), (0, ""));
    assert!(run.out.contains("Saved: 0 switched off in ") && run.out.contains(", 2 cleared and 0 still waiting in "), "{}", run.out);
    assert_eq!(steps.moved_to, ["s.zip.waiting.json"], "only the waiting file was saved");
    assert_eq!(std::fs::read(&file).unwrap(), zip_before, "the zip is byte for byte the same");
    assert!(records_of(&file).is_empty());
}

/// The UTC text of the second before `moment`.
fn previous_second(moment: &str) -> String {
    let ms = run_capture::moment(moment) - 1000;
    leko_core::sessions::utc_moment::format_utc_moment(ms).expect("a moment")
}
