//! The wall clock the rules are judged on is the zone `--zone` names, else the computer's zone that
//! `main.rs` hands in (AC-004). A zone that is unknown, empty or missing gives no answer and is never
//! read as UTC: the same outside-hours rule gives a different answer in each of three zones.

mod run_capture;
mod scratch_folder;
mod settings_builder;

use run_capture::{status, Outcome};
use scratch_folder::Scratch;
use serde_json::json;
use settings_builder::{block, outside_hours, write_settings};
use std::path::{Path, PathBuf};

const NOW: &str = "2026-11-05T15:00:00Z";
const ASK_FOR_ZONE: &str = "Name the zone with --zone, for example --zone America/New_York.";

fn office_hours() -> (Scratch, PathBuf) {
    let folder = Scratch::new("zone");
    let file = write_settings(folder.path(), "settings.zip", &[block("hours", json!({"apps": ["com.h"]}), json!([outside_hours("r", &[], &[("09:00", "17:00")])]))]);
    (folder, file)
}

fn ask(file: &Path, zone_flag: Option<&str>, computer: Option<&str>) -> Outcome {
    let mut flags = vec!["--app", "com.h"];
    flags.extend(zone_flag.into_iter().flat_map(|name| ["--zone", name]));
    status(file, &flags, NOW, computer)
}

fn sentence_of(run: &Outcome) -> &str {
    assert_eq!((run.code, run.err.as_str()), (0, ""), "{}", run.err);
    &run.out
}

fn nothing_judged(run: &Outcome, stderr: &str) {
    assert_eq!((run.code, run.out.as_str()), (1, ""), "{}", run.err);
    assert_eq!(run.err, stderr);
}

#[test]
fn the_zone_flag_beats_the_computers_zone_and_an_unknown_or_missing_zone_gives_no_answer() {
    let (_folder, file) = office_hours();
    let tail = "Apps that are never blocked are not applied yet. Nothing was saved or changed.\n";

    // The same instant and the same rule, in three zones: 10:00 in New York (inside the hours), 00:00 in
    // Tokyo (outside), 15:00 in UTC (inside). UTC is not what an unnamed zone turns into.
    let new_york = ask(&file, Some("America/New_York"), Some("Asia/Tokyo"));
    assert_eq!(sentence_of(&new_york), format!("App \"com.h\" is open until 2026-11-05T17:00:00-05:00, judged at 2026-11-05T10:00:00-05:00 in America/New_York. {tail}"), "the flag beats the computer's zone");
    let tokyo = ask(&file, None, Some("Asia/Tokyo"));
    assert_eq!(sentence_of(&tokyo), format!("App \"com.h\" is closed until 2026-11-06T09:00:00+09:00, judged at 2026-11-06T00:00:00+09:00 in Asia/Tokyo. Closed by block \"hours\" rule \"r\" (outsideHours). {tail}"), "with no flag the computer's zone is used");
    let utc = ask(&file, Some("UTC"), Some("Asia/Tokyo"));
    assert_eq!(sentence_of(&utc), format!("App \"com.h\" is open until 2026-11-05T17:00:00Z, judged at 2026-11-05T15:00:00Z in UTC. {tail}"));
    let kolkata = ask(&file, None, Some("Asia/Kolkata"));
    assert_eq!(sentence_of(&kolkata), format!("App \"com.h\" is closed until 2026-11-06T09:00:00+05:30, judged at 2026-11-05T20:30:00+05:30 in Asia/Kolkata. Closed by block \"hours\" rule \"r\" (outsideHours). {tail}"));
    assert_eq!(sentence_of(&ask(&file, Some("Asia/Tokyo"), None)), tokyo.out, "a flag needs no computer zone");
    assert_eq!(sentence_of(&ask(&file, Some("America/New_York"), Some("Nonsense/Zone"))), new_york.out, "a bad computer zone does not matter when the flag names one");

    // The flag is what counts even when it is wrong: a computer zone does not stand in for it.
    let wrong = |name: &str| format!("the time zone \"{name}\" is not one this computer knows\n");
    for name in ["Mars/Base", "", "america/new_york", "New York", "UTC+5", " UTC"] {
        nothing_judged(&ask(&file, Some(name), Some("America/New_York")), &wrong(name));
        nothing_judged(&ask(&file, Some(name), None), &wrong(name));
    }

    // No flag, and a computer zone that is missing or cannot be used: a sentence asking for --zone.
    nothing_judged(&ask(&file, None, None), &format!("This computer's time zone could not be found, so nothing was judged. {ASK_FOR_ZONE}\n"));
    for name in ["Nonsense/Zone", "", "Mars/Base"] {
        let sentence = format!("This computer's time zone \"{name}\" is not one that can be used, so nothing was judged. {ASK_FOR_ZONE}\n");
        nothing_judged(&ask(&file, None, Some(name)), &sentence);
    }

    // The zone is settled before the file is opened, and with --json nothing is printed either.
    let missing = Path::new("/no/such/settings.zip");
    nothing_judged(&status(missing, &["--app", "com.h", "--zone", "Mars/Base"], NOW, Some("UTC")), &wrong("Mars/Base"));
    nothing_judged(&status(&file, &["--json", "--app", "com.h", "--zone", "Mars/Base"], NOW, None), &wrong("Mars/Base"));
}
