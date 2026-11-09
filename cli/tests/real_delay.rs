//! The real `leko` program, started as a process with the real clock and the zone from `TZ`, does the
//! whole path on built zips and on the phone-exported zips of the corpus (AC-016): switches off a block
//! that nothing locks, one whose seal has ended, parks a locked one, lists it, and lands it once its
//! moment has passed. Results go to stdout, errors to stderr, with the codes 0, 1 and 2.

mod delay_runs;
#[path = "../../core/tests/corpus_zips/mod.rs"]
mod corpus_zips;
#[path = "../../core/tests/delay_sets/mod.rs"]
mod delay_sets;
mod run_capture;
mod scratch_folder;
#[path = "../../core/tests/zip_writer/mod.rs"]
mod zip_writer;

use corpus_zips::{case_bytes, load_corpus};
use delay_runs::{assert_core_reads, assert_switched_off, block_of, one_line, record, records_of, waiting_of, write_zip};
use delay_sets::{locked_block, sealed_at, sealed_until, wait};
use leko_cli::session::session_arguments::USAGE;
use leko_core::config_time::zone_time::ZoneTime;
use leko_core::delayed_changes::switch_off_plan::{plan_switch_off, SwitchOffPlan};
use leko_core::io::zip_check::{check_zip, ZipVerdict};
use leko_core::sessions::utc_moment::{format_utc_moment, parse_utc_moment};
use run_capture::{outcome_of, Outcome};
use scratch_folder::{run_tz, Scratch};
use serde_json::Value;
use std::collections::HashSet;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
use zip_writer::Method;

const HOUR_MS: i64 = 3_600_000;

/// `leko ARGS...` as a process on a computer whose zone is `tz`.
fn leko(args: &[&str], tz: &str) -> Outcome {
    outcome_of(run_tz(args, tz))
}

fn path_text(path: &Path) -> &str {
    path.to_str().expect("a text path")
}

fn clock_ms() -> i64 {
    i64::try_from(SystemTime::now().duration_since(UNIX_EPOCH).expect("a clock after 1970").as_millis()).expect("a moment that fits")
}

fn moment_text(ms: i64) -> String {
    format_utc_moment(ms.div_euclid(1000) * 1000).expect("a moment")
}

fn done(run: &Outcome, what: &str) {
    assert_eq!((run.code, run.err.as_str()), (0, ""), "{what}: {}", run.out);
}

#[test]
fn the_real_program_switches_off_parks_lists_and_lands() {
    let folder = Scratch::new("real-delay");
    let blocks = [
        locked_block("free", Value::Null),
        locked_block("ended", sealed_until("2020-01-01")),
        locked_block("sealed", sealed_at("2020-01-01T00:00:00Z")),
        locked_block("slow", wait(1)),
    ];
    let file = write_zip(folder.path(), "s.zip", &blocks, &[]);
    let name = path_text(&file);
    let waiting = waiting_of(&file);

    // Nothing locks `free`, and the seals of `ended` and `sealed` have ended: each lands at once.
    let before = std::fs::read(&file).unwrap();
    let draft = leko(&["switch-off", name, "--id", "free"], "UTC");
    done(&draft, "draft");
    assert!(draft.out.starts_with("Draft: the block \"free\" would be switched off now. Nothing was saved.\n"), "{}", draft.out);
    assert_eq!(std::fs::read(&file).unwrap(), before, "a draft saves nothing");
    for id in ["free", "ended", "sealed"] {
        let old = block_of(&file, id);
        let run = leko(&["switch-off", name, "--id", id, "--write"], "UTC");
        done(&run, id);
        assert!(run.out.starts_with(&format!("Saved: the block \"{id}\" is switched off in ")), "{}", run.out);
        assert_switched_off(&old, &block_of(&file, id));
        assert_core_reads(&file);
    }
    assert_eq!(folder.names(), ["s.zip"], "nothing but the zip was made");
    let again = leko(&["switch-off", name, "--id", "free", "--write"], "UTC");
    done(&again, "again");
    assert_eq!(again.out, "The block \"free\" is already off. Nothing was saved.\n");

    // `slow` waits an hour: it is parked at the real moment, in the zone of this computer.
    let zip_before = std::fs::read(&file).unwrap();
    let asked_after = clock_ms();
    let parked = leko(&["switch-off", name, "--id", "slow", "--write"], "Asia/Tokyo");
    let asked_before = clock_ms();
    done(&parked, "park");
    assert!(parked.out.starts_with("Waiting: the block \"slow\" is not switched off yet. It may be switched off from "), "{}", parked.out);
    assert_eq!(std::fs::read(&file).unwrap(), zip_before, "the zip is untouched while the change waits");
    let records = records_of(&file);
    assert_eq!(records.len(), 1);
    let requested = parse_utc_moment(records[0]["requestedAt"].as_str().unwrap()).unwrap();
    let due = parse_utc_moment(records[0]["dueAt"].as_str().unwrap()).unwrap();
    assert!((asked_after.div_euclid(1000) * 1000..=asked_before).contains(&requested), "the request moment is the real clock: {requested} not in {asked_after}..{asked_before}");
    assert_eq!(due, requested + HOUR_MS, "a wait of one hour");
    let tokyo = ZoneTime::new("Asia/Tokyo").unwrap().local_text(due);
    assert!(tokyo.ends_with("+09:00") && parked.out.contains(&format!("from {tokyo} (Asia/Tokyo)")), "{}", parked.out);

    // Asking again changes nothing; settle lists the change in the zone of the computer or of --zone.
    let waiting_before = std::fs::read(&waiting).unwrap();
    let twice = leko(&["switch-off", name, "--id", "slow", "--write"], "Asia/Tokyo");
    done(&twice, "twice");
    assert!(twice.out.starts_with("The block \"slow\" is already waiting: it may be switched off from "), "{}", twice.out);
    for (flags, zone, local_zone) in [(&[][..], "UTC", "UTC"), (&[], "Asia/Tokyo", "Asia/Tokyo"), (&["--zone", "America/New_York"], "Asia/Tokyo", "America/New_York")] {
        let local = ZoneTime::new(local_zone).unwrap().local_text(due);
        let listed = leko(&[&["settle", name][..], flags, &["--write"]].concat(), zone);
        done(&listed, local_zone);
        assert!(listed.out.starts_with(&format!("The block \"slow\" is not due yet: it may be switched off from {local} ({local_zone}), held by: ")), "{}", listed.out);
        assert!(listed.out.ends_with("Nothing is due yet. Nothing was saved.\n"), "{}", listed.out);
    }
    assert_eq!(std::fs::read(&waiting).unwrap(), waiting_before, "listing and asking again left the waiting file alone");
    assert_eq!(std::fs::read(&file).unwrap(), zip_before);

    // Two hours later (the waiting file says so), the change lands and the list is kept, empty.
    let now = clock_ms();
    std::fs::write(&waiting, serde_json::json!({"format": 1, "changes": [record("slow", &moment_text(now - 2 * HOUR_MS), &moment_text(now - HOUR_MS))]}).to_string()).unwrap();
    let old = block_of(&file, "slow");
    let landed = leko(&["settle", name, "--write"], "UTC");
    done(&landed, "land");
    assert!(landed.out.starts_with("The block \"slow\" is switched off.\nSaved: 1 switched off in "), "{}", landed.out);
    assert_switched_off(&old, &block_of(&file, "slow"));
    assert_core_reads(&file);
    assert!(records_of(&file).is_empty());
    let nothing = leko(&["settle", name, "--write"], "UTC");
    done(&nothing, "nothing");
    assert_eq!(nothing.out, "Nothing is waiting. Nothing was saved.\n");

    // Results are on stdout, problems on stderr, with the code that says which kind they are.
    let other = write_zip(folder.path(), "o.zip", &[locked_block("a", Value::Null)], &[]);
    let other_name = path_text(&other);
    for args in [&["switch-off", other_name][..], &["settle", other_name, "--id", "a"], &["switch-off", other_name, "--id", "a", "--zone", "Nowhere/Land"], &["settle", other_name, "--bogus"]] {
        let run = leko(args, "UTC");
        assert_eq!((run.code, run.out.as_str()), (2, ""), "{args:?}: {}", run.err);
        assert!(run.err.ends_with(&format!("{USAGE}\n")), "{args:?}: {}", run.err);
    }
    let missing = path_text(&folder.path().join("missing.zip")).to_string();
    for args in [vec!["switch-off", &missing, "--id", "a"], vec!["settle", &missing], vec!["switch-off", other_name, "--id", "nope"]] {
        let run = leko(&args, "UTC");
        assert_eq!((run.code, run.out.as_str()), (1, ""), "{args:?}: {}", run.err);
        assert!(one_line(&run.err).ends_with(". Nothing was saved.\n"), "{args:?}: {}", run.err);
    }
    let no_zone = leko(&["switch-off", other_name, "--id", "a"], "Not/AZone");
    assert_eq!((no_zone.code, no_zone.out.as_str()), (1, ""), "{}", no_zone.err);
    assert!(one_line(&no_zone.err).contains("--zone") && !no_zone.err.contains("Usage"), "{}", no_zone.err);
    done(&leko(&["switch-off", other_name, "--id", "a", "--zone", "UTC"], "Not/AZone"), "--zone beats an unusable TZ");
    assert_eq!(block_of(&other, "a")["on"], Value::Null, "only drafts and refusals were made on this file");
}

#[test]
fn the_real_program_works_on_the_phone_exported_zips() {
    let utc = ZoneTime::new("UTC").unwrap();
    let (mut landed, mut parked) = (0, 0);
    for item in load_corpus().cases.iter().filter(|item| item["expect"].get("read").is_some()) {
        let (case, bytes) = (item["name"].as_str().unwrap_or("?"), case_bytes(item, Method::Deflate));
        let ZipVerdict::Read(read) = check_zip(&bytes, &HashSet::new()) else { continue };
        let ids: Vec<String> = read.set.docs.iter().filter_map(|(path, _)| path.strip_prefix("blocks/")?.strip_suffix(".json").map(str::to_string)).collect();
        let now = clock_ms();
        let lands = ids.iter().find(|id| plan_switch_off(&read.set, id, now, &utc) == SwitchOffPlan::LandsNow);
        let waits = ids.iter().find(|id| matches!(plan_switch_off(&read.set, id, now, &utc), SwitchOffPlan::Waits { .. }));
        for (id, wanted_landing) in lands.map(|id| (id, true)).into_iter().chain(waits.map(|id| (id, false))) {
            let folder = Scratch::new("real-corpus");
            let file = folder.path().join("s.zip");
            std::fs::write(&file, &bytes).unwrap();
            let old = block_of(&file, id);
            let run = leko(&["switch-off", path_text(&file), "--id", id, "--write"], "UTC");
            done(&run, &format!("{case} / {id}"));
            assert_core_reads(&file);
            if wanted_landing {
                assert_switched_off(&old, &block_of(&file, id));
                assert_eq!(folder.names(), ["s.zip"], "{case} / {id}");
                landed += 1;
            } else {
                assert!(run.out.starts_with(&format!("Waiting: the block \"{id}\" is not switched off yet.")), "{case} / {id}: {}", run.out);
                assert_eq!(block_of(&file, id), old, "{case} / {id}: the block waits");
                assert_eq!(folder.names(), ["s.zip", "s.zip.waiting.json"], "{case} / {id}");
                assert_eq!(records_of(&file).len(), 1);
                parked += 1;
            }
        }
    }
    assert!(landed >= 3 && parked >= 1, "the corpus was used: {landed} landed, {parked} parked");
}
