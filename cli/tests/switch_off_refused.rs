//! A request the core cannot honour exits 1 with one line ending ". Nothing was saved." and saves
//! nothing (AC-007): an id no block has (a path-shaped one is only unknown and never builds a path),
//! a block whose applied lock has a partner's key, and locks whose change windows share no minute.

mod parked_change_support;
use parked_change_support::{delay_runs, delay_sets, run_capture, scratch_folder, zip_writer};
use delay_runs::{one_line, snapshot, switch_off, write_zip, NOW};
use delay_sets::{change_hours, locked_block, routine_holding, wait, with_partner_key};
use scratch_folder::Scratch;
use serde_json::Value;

fn assert_refused(run: &run_capture::Outcome, reason: &str, what: &str) {
    assert_eq!((run.code, run.out.as_str()), (1, ""), "{what}: {}", run.err);
    let line = one_line(&run.err);
    assert!(line.ends_with(". Nothing was saved.\n") && line.contains(reason), "{what}: {line}");
}

#[test]
fn each_refusal_is_one_line_names_the_reason_and_saves_nothing() {
    // The zip is one folder down, so a path built from an id would land in the outer folder.
    let outer = Scratch::new("refused");
    std::fs::create_dir(outer.path().join("inner")).unwrap();
    let inner = outer.path().join("inner");
    let blocks = [
        locked_block("a", Value::Null),
        locked_block("keyed", with_partner_key(wait(24))),
        locked_block("routine-keyed", wait(24)),
        locked_block("never", change_hours("08:00", "09:00", 0)),
        locked_block("B", Value::Null),
    ];
    let routines = [routine_holding("k", &["routine-keyed"], with_partner_key(wait(1))), routine_holding("w", &["never"], change_hours("20:00", "21:00", 0))];
    let file = write_zip(&inner, "s.zip", &blocks, &routines);
    let (before_inner, before_outer) = (snapshot(&inner), outer.names());

    let cases: [(&str, &str, &str); 11] = [
        ("an unknown id", "nope", r#"There is no block "nope" in "#),
        ("an id with a hyphen no block has", "a-b", r#"There is no block "a-b" in "#),
        ("a path with .. and a separator", "../x", r#"There is no block "../x" in "#),
        ("a longer path", "a/../../x", r#"There is no block "a/../../x" in "#),
        ("an absolute path", "/etc/passwd", r#"There is no block "/etc/passwd" in "#),
        ("a name with a line break", "a\nb", r#"There is no block "a\nb" in "#),
        ("a name with a quote", "a\"b", r#"There is no block "a\"b" in "#),
        ("a partner key on the block", "keyed", r#"A lock on the block "keyed" needs a partner's key"#),
        ("a partner key on a routine that holds it", "routine-keyed", r#"A lock on the block "routine-keyed" needs a partner's key"#),
        ("windows that share no minute", "never", r#"The locks on the block "never" never leave a moment to change it"#),
        ("an id with a trailing space", "a ", r#"There is no block "a " in "#),
    ];
    for (what, id, reason) in cases {
        for flags in [&["--id", id][..], &["--id", id, "--write"]] {
            let run = switch_off(&file, flags, NOW);
            assert_refused(&run, reason, &format!("{what} {flags:?}"));
            assert_eq!(snapshot(&inner), before_inner, "{what} {flags:?}: nothing was saved");
            assert_eq!(outer.names(), before_outer, "{what} {flags:?}: no file was made outside");
        }
    }
    // Ids are exact: a block named with a capital letter is its own block, and works.
    let run = switch_off(&file, &["--id", "B"], NOW);
    assert_eq!((run.code, run.err.as_str()), (0, ""));
}
