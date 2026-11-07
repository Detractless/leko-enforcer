//! Taking a parked change back (AC-001): only the record of the block named goes, the block id is
//! compared exactly (a prefix, another case or path-shaped text matches nothing), and every other
//! record comes back as it was, in the same order.

use leko_core::delayed_changes::waiting_change::{ChangeKind, WaitingChange};
use leko_core::waiting_changes::cancel_waiting::cancel_waiting;

const HOUR_MS: i64 = 3_600_000;
const START: i64 = 1_793_000_000_000;

fn change(block: &str, asked_hours_after_start: i64, wait_hours: i64) -> WaitingChange {
    let requested = START + asked_hours_after_start * HOUR_MS;
    WaitingChange::new(ChangeKind::SwitchOff, block, requested, requested + wait_hours * HOUR_MS).expect("a good record")
}

#[test]
fn cancelling_removes_only_that_blocks_record_and_keeps_the_rest_in_order() {
    let (first, middle, last) = (change("a", 1, 24), change("ab", 2, 48), change("b", 3, 1));
    let list = [first.clone(), middle.clone(), last.clone()];

    // The middle one goes; the others are exactly as they were and in the same order.
    let done = cancel_waiting(&list, "ab");
    assert_eq!(done.removed, Some(middle.clone()));
    assert_eq!(done.remaining, [first.clone(), last.clone()]);

    // The first and the last, one at a time; `a` is not a prefix match for `ab`.
    let done = cancel_waiting(&list, "a");
    assert_eq!((done.removed, done.remaining), (Some(first.clone()), vec![middle.clone(), last.clone()]));
    let done = cancel_waiting(&list, "b");
    assert_eq!((done.removed, done.remaining), (Some(last.clone()), vec![first.clone(), middle.clone()]));

    // Text that is not exactly a block id in the list matches nothing, and the list comes back whole.
    for odd in ["", "A", "AB", "a ", " a", "abc", "../a", "a/../b", "a\0", "*"] {
        let done = cancel_waiting(&list, odd);
        assert_eq!((done.removed, done.remaining.as_slice()), (None, &list[..]), "{odd:?}");
    }

    // The last record of a list leaves an empty list, and an empty list stays empty.
    let one = [first.clone()];
    let done = cancel_waiting(&one, "a");
    assert_eq!((done.removed, done.remaining), (Some(first), vec![]));
    let done = cancel_waiting(&[], "a");
    assert_eq!((done.removed, done.remaining), (None, vec![]));
}
