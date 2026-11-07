//! A block the settings do not have is refused (AC-007): exit 1, one line ending ". Nothing was
//! saved." on stderr, nothing on stdout, and no file touched or made. An id with a separator, `..`, a
//! capital letter or an odd character is only a block that is not there: it never builds a path, and a
//! near match (a prefix, another case) never switches on a different block.

mod delay_runs;
#[path = "../../core/tests/delay_sets/mod.rs"]
mod delay_sets;
mod run_capture;
mod scratch_folder;
#[path = "../../core/tests/zip_writer/mod.rs"]
mod zip_writer;

use delay_runs::{block_of, one_line, snapshot, switch_on, write_zip};
use delay_sets::{locked_block, off_block};
use scratch_folder::Scratch;
use serde_json::{json, Value};

fn off_cased(id: &str) -> Value {
    let mut file = locked_block(id, Value::Null);
    file["on"] = json!(false);
    file
}

#[test]
fn an_unknown_or_path_shaped_id_is_refused_and_nothing_is_touched() {
    // The zip is one folder down, so a path built from an id would land in the outer folder.
    let outer = Scratch::new("switch-on-missing");
    std::fs::create_dir(outer.path().join("inner")).unwrap();
    let inner = outer.path().join("inner");
    let file = write_zip(&inner, "s.zip", &[off_block("a"), off_block("ab"), off_cased("B")], &[]);
    let (before_inner, before_outer) = (snapshot(&inner), outer.names());

    let cases: [(&str, &str); 14] = [
        ("an unknown id", "nope"),
        ("an id with a hyphen no block has", "a-b"),
        ("a capital where the block has a small letter", "A"),
        ("a small letter where the block has a capital", "b"),
        ("a prefix of another block", "a_"),
        ("a path with .. and a separator", "../x"),
        ("a longer path", "a/../../x"),
        ("an absolute path", "/etc/passwd"),
        ("a path to the settings file itself", "../inner/s.zip"),
        ("a name with a line break", "a\nb"),
        ("a name with a quote", "a\"b"),
        ("a trailing space", "a "),
        ("a block file name", "blocks/a.json"),
        ("a dot", "."),
    ];
    for (what, id) in cases {
        for flags in [&["--id", id][..], &["--id", id, "--write"]] {
            let run = switch_on(&file, flags);
            assert_eq!((run.code, run.out.as_str()), (1, ""), "{what} {flags:?}: {}", run.err);
            let line = one_line(&run.err);
            assert!(line.starts_with(&format!("There is no block {} in ", leko_cli::session::session_report::shown(id))) && line.ends_with(". Nothing was saved.\n"), "{what} {flags:?}: {line}");
            assert!(!line.contains("panicked") && !line.contains("RUST_BACKTRACE"), "{what}: a sentence, not a stack trace");
            assert_eq!(snapshot(&inner), before_inner, "{what} {flags:?}: nothing was saved");
            assert_eq!(outer.names(), before_outer, "{what} {flags:?}: no file was made outside");
        }
    }

    // Ids are exact: a block named with a capital letter is its own block, and works; `a` and `ab`
    // are two blocks and switching one on leaves the other off.
    let run = switch_on(&file, &["--id", "B", "--write"]);
    assert_eq!((run.code, run.err.as_str()), (0, ""));
    assert_eq!(block_of(&file, "B")["on"], true);
    assert_eq!(block_of(&file, "a")["on"], false);
    assert_eq!(block_of(&file, "ab")["on"], false);
}
