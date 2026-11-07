//! The core builds the block from the request alone, and the same request gives the same block
//! (AC-003): the id comes from the name by a fixed rule, a name that cannot make a safe id is refused
//! with a sentence and no block, and what the drafter writes is a file the readers accept.

mod zip_writer;

use leko_core::blocks::add_block::{add_block, block_path};
use leko_core::blocks::block_id::id_from_name;
use leko_core::blocks::block_request::{BlockRequest, DoorChoice, RuleChoice};
use leko_core::blocks::draft_block::draft_block;
use serde_json::json;
use zip_writer::zip_of;

const NOW: i64 = 1_793_000_000_000;

fn request(name: &str) -> BlockRequest {
    BlockRequest { name: name.to_string(), apps: vec!["com.example.app".to_string()], sites: vec![], rule: RuleChoice::Always, days: vec![], door: DoorChoice::Default, message: None }
}

fn empty_settings() -> Vec<u8> {
    zip_of(&[("GUIDE.md", "guide"), ("general.json", r#"{"schema": 2}"#)])
}

#[test]
fn the_same_request_makes_the_same_block_and_the_id_comes_from_the_name() {
    // The same request at the same moment is the same bytes, whatever else ran in between.
    let mut timed = request("Study");
    timed.rule = RuleChoice::ForMinutes(90);
    for ask in [request("Social Media"), timed] {
        let first = draft_block(&ask, NOW).expect("a good request is drafted");
        let _other = draft_block(&request("Something else"), NOW + 1);
        let second = draft_block(&ask, NOW).expect("a good request is drafted");
        assert_eq!(serde_json::to_string(&first.block).unwrap(), serde_json::to_string(&second.block).unwrap());
        assert_eq!(first, second);
    }

    // The whole file for a plain request, in the order the writer keeps.
    let plain = draft_block(&request("Social Media"), NOW).unwrap();
    assert_eq!(plain.id, "social-media");
    assert_eq!(plain.ends_at, None);
    assert_eq!(
        serde_json::to_string(&plain.block).unwrap(),
        r#"{"schema":2,"id":"social-media","name":"Social Media","on":true,"covers":{"apps":["com.example.app"]},"rules":[{"id":"always","kind":"always"}],"door":{"kind":"wait","seconds":30},"pass":{"length":"minutes","minutes":10,"perDay":3}}"#
    );

    // The id rule: letters and digits lowercased, every run of anything else one dash, none at the ends.
    let ids = [
        ("Social Media", "social-media"),
        ("social-media", "social-media"),
        ("  --Hi!!   there__ ", "hi-there"),
        ("A_B", "a-b"),
        ("2026 Plan", "2026-plan"),
        ("Caf\u{e9} au lait", "caf-au-lait"),
        ("x", "x"),
        ("Games & TV (evenings)", "games-tv-evenings"),
    ];
    for (name, id) in ids {
        assert_eq!(id_from_name(name), Ok(id.to_string()), "{name:?}");
        assert_eq!(draft_block(&request(name), NOW).unwrap().id, id, "{name:?}");
    }
    let longest = "a".repeat(60);
    assert_eq!(id_from_name(&longest), Ok(longest.clone()));

    // Names that are refused: no block, a sentence that says why.
    let refused = [
        ("", "needs a name"),
        ("!!!", "letter"),
        ("\u{65e5}\u{672c}\u{8a9e}", "letter"),
        (&"a".repeat(61), "60"),
        ("two\nlines", "control"),
        ("esc\u{1b}[0m", "control"),
        ("tab\there", "control"),
        ("del\u{7f}", "control"),
    ];
    for (name, word) in refused {
        let sentence = id_from_name(name).expect_err(name);
        assert!(sentence.contains(word), "{name:?}: {sentence}");
        assert_eq!(draft_block(&request(name), NOW).err(), Some(sentence), "{name:?}: the drafter gives the same sentence");
    }
    // Length is counted in UTF-16 units as the phone counts: an emoji is two.
    assert!(id_from_name(&format!("{}a", "\u{1f600}".repeat(29))).is_ok(), "59 units");
    assert!(id_from_name(&format!("{}a", "\u{1f600}".repeat(30))).is_err(), "61 units");

    // What is written is a file the readers accept, under the id, as its own entry.
    let mut message = request("Reading hour");
    message.message = Some("Back to the book.".to_string());
    for ask in [request("Social Media"), message, request(&longest)] {
        let draft = draft_block(&ask, NOW).unwrap();
        let added = add_block(&empty_settings(), &draft).unwrap_or_else(|problem| panic!("{}", problem.sentence()));
        let files = leko_core::open_zip(&added.bytes).unwrap().files;
        let written = files.iter().find(|(path, _)| *path == block_path(&draft.id)).expect("the block is its own file");
        assert_eq!(serde_json::from_slice::<serde_json::Value>(&written.1).unwrap(), draft.block);
    }

    // The other parts of a request are refused with a sentence too, and no block is made.
    let mut nothing = request("Nothing");
    nothing.apps.clear();
    let mut empty_message = request("Msg");
    empty_message.message = Some(String::new());
    let mut long_message = request("Msg");
    long_message.message = Some("m".repeat(301));
    let mut control_message = request("Msg");
    control_message.message = Some("bad\u{7}".to_string());
    for bad in [nothing, empty_message, long_message, control_message] {
        assert!(draft_block(&bad, NOW).is_err(), "{bad:?}");
    }
    let mut longest_message = request("Msg");
    longest_message.message = Some("m".repeat(300));
    assert_eq!(draft_block(&longest_message, NOW).unwrap().block["message"], json!("m".repeat(300)));
}
