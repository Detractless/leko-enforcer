# Contract: switch-on

Written before any code. After the user opens it, the contract is locked: criteria cannot be edited to match the code.
Every "<...>" part must be replaced with real text. `/contract open <feature>` refuses a contract that is not complete.

## Goal
The user can switch a block back on with `leko switch-on FILE --id ID [--write]`. Switching on is a tightening, so it lands at once: no wait, no partner key, whatever locks the block has. It rewrites only the settings zip, and it refuses while a switch-off of the same block is still parked, so the two cannot undo each other.

## Domains
Domains: cli, data

## Acceptance criteria
- AC-001: The core decides a switch-on from the settings alone: `plan_switch_on(set, id)` says there is no such block, the block is already on (an `on` of true, or no `on` field), or the switch-on lands now; it never says "waits", whatever locks apply (none, a wait, change hours outside their window, a seal until a day or a moment, a partner's key, or the lock of a routine that holds the block); and the core's loosening judge, asked about `"on": false` becoming `"on": true`, finds nothing that loosens.
  - evidence: core/tests/switch_on_plan.rs "a_switch_on_lands_at_once_whatever_locks_the_block_has"
  - breaker: A lock or a key is applied to a tightening so the block waits or is refused, a block with no `on` field is treated as off, or the table is read the wrong way round so switching on counts as a loosening.
- AC-002: `switch_block_on(bytes, id)` returns the zip with `"on": true` written into that block's file and nothing else changed: every other field of the block, every other block and every other file keep their names, contents and order; the new zip is read again by the core before it is handed back; the input is not changed; and it works on a zip made by `leko add-block`, on one made by `leko switch-off`, and on the phone-exported zips of the corpus. A missing block, a block already on, and a zip the core refuses give a sentence and no bytes.
  - evidence: core/tests/block_on_zip.rs "only_the_on_field_of_that_block_changes_and_the_new_zip_is_read_again"
  - breaker: Another block or file is touched or reordered, the field is removed instead of set so the file changes shape, `on` stays false, or a zip that the reader would refuse is handed back.
- AC-003: `leko switch-on FILE --id ID` prints one plain draft and writes nothing (settings zip, waiting file, folder and home folder as they were), exit 0: it says the block would be switched on now, ends with "Nothing was saved" and gives the same command with `--write`.
  - evidence: cli/tests/switch_on_draft.rs "without_the_write_flag_the_draft_says_what_would_happen_and_saves_nothing"
  - breaker: The draft already changes the zip, leaves a temporary file, or does not say which block it is about.
- AC-004: With `--write` the settings zip is replaced by the zip of AC-002: only that block's `on` changes, every other entry is byte for byte as it was, the sentence says "Saved" and names the file, the saved zip passes the core's reader, and the waiting file is neither written nor made.
  - evidence: cli/tests/switch_on_write.rs "write_changes_only_that_blocks_on_field_and_leaves_the_waiting_file_alone"
  - breaker: The wrong block is switched on, a neighbour changes, the waiting file is rewritten or created, or the saved file is one the reader refuses.
- AC-005: Switching on is never held back: it lands at once, with no wait and no key, for a block whose lock is a long wait, a seal until a far day or moment, a partner's key, change hours outside their window, or a block held by a routine with such a lock.
  - evidence: cli/tests/switch_on_free.rs "a_switch_on_is_never_held_by_a_lock_or_a_key"
  - breaker: A switch-on is parked, asked for a partner key, or refused because the block's lock is closed, so the user cannot tighten their own rules.
- AC-006: Running it again, or on a block that is already on (an `on` of true, or no `on` field), is safe and says what it did: exit 0 with "The block ... is already on. Nothing was saved." for the draft and for `--write`, the zip byte for byte as it was, no file made; the same `--write` typed twice leaves the second run a no-op.
  - evidence: cli/tests/switch_on_repeat.rs "a_second_switch_on_or_a_block_already_on_changes_nothing_and_says_why"
  - breaker: A second run errors or rewrites the zip, a block with no `on` field is "switched on" by writing a field it did not need, or success is reported for a write that did not happen.
- AC-007: A block that is not in the settings gives exit 1 and one sentence ending "Nothing was saved."; an id with `/`, `..` or capital letters is just a block that is not there and never builds a path; no file is touched or made.
  - evidence: cli/tests/switch_on_missing.rs "an_unknown_or_path_shaped_id_is_refused_and_nothing_is_touched"
  - breaker: A path-shaped id reaches the file system, a near match (a prefix, another case) switches on a different block, or the refusal is a stack trace.
- AC-008: While the waiting file holds a parked switch-off of the same block, `switch-on` (draft and `--write`, whether the block is on or off) exits 1 with one sentence that says the waiting change would undo it and names `leko cancel FILE --id ID --write`; nothing is saved. A parked change of another block does not stop it, and after the cancel the switch-on goes through.
  - evidence: cli/tests/switch_on_parked.rs "a_parked_switch_off_of_the_same_block_stops_a_switch_on_until_it_is_cancelled"
  - breaker: A block is switched on and then switched off again by `leko settle` later, another block's parked change blocks the command, or the sentence does not say what to do.
- AC-009: Wrong or missing arguments print the usage line (which now lists `leko switch-on`) to stderr and exit 2, with nothing on stdout and no file touched: no file, no `--id`, `--id` twice, `--id` with nothing after it, an unknown flag (including `--zone` and `--json`, which switch-on has no use for), a second file, and `--write` given twice.
  - evidence: cli/tests/switch_on_arguments.rs "bad_arguments_print_usage_and_exit_two"
  - breaker: A missing `--id` switches on the first block, an unknown flag is ignored, usage goes to stdout, or the usage line forgets the new command.
- AC-010: A settings file that is missing, empty, not a zip or refused by the core gives exit 1 and one plain sentence ending "Nothing was saved."; a waiting file that is empty, not JSON, not text, of a newer format, holds a record the reader cannot read, is a folder, or is over 1 MB gives exit 1 and one plain sentence; neither file is replaced, repaired or made; a waiting file that is not there is not a problem.
  - evidence: cli/tests/switch_on_files.rs "a_bad_settings_or_waiting_file_is_reported_and_left_alone"
  - breaker: A corrupt waiting file is read as "nothing parked" so a parked loosening is missed, a damaged zip is replaced by a clean one, a missing waiting file is made, or the sentence is a stack trace.
- AC-011: The save goes through the temporary-file-then-move step used by the other commands: a failure at each step (write, flush, move) leaves the settings zip exactly as it was, leaves no temporary file, and exits 1 with one sentence naming the step; the same command run again with a working disk finishes the job; a draft, a no-op and a refusal never reach a step.
  - evidence: cli/tests/switch_on_atomic_save.rs "a_failure_at_any_step_leaves_the_settings_zip_as_it_was"
  - breaker: A failed save truncates the zip, leaves a half-written file or a temporary file, reports success, or a command that saves nothing still tries to save.
- AC-012: The real `leko` program does the whole path on built zips: switch a block off, see in `status` that its app is open, `switch-on --write` it and see in `status` that it is closed again; a locked block with a parked switch-off is refused (exit 1), then `cancel --write` clears the way and the switch-on goes through; results go to stdout, errors to stderr, and exit codes are 0 for done or nothing to do, 1 for refused, 2 for not understood.
  - evidence: cli/tests/real_switch_on.rs "the_real_program_switches_off_and_on_again_and_status_follows"
  - breaker: The built program differs from the tested functions (command not wired), an error goes to stdout, `status` still shows the block open after a switch-on, or an exit code does not say which kind of result it was.

## Checklist coverage
- CLI-1 -> criterion:AC-009
- CLI-2 -> criterion:AC-012
- CLI-3 -> criterion:AC-006
- DATA-1 -> criterion:AC-011
- DATA-2 -> criterion:AC-010
- DATA-3 -> criterion:AC-002

## Derived checklist
Not used: Domains is not "none".

## Edge cases
- A block with no `on` field, which the phone reads as on -> AC-006
- A block whose lock is a seal, a long wait or a partner's key -> AC-005
- A block held by a routine whose lock is closed -> AC-005
- A parked switch-off of the same block, and one of a different block -> AC-008
- An id with `/`, `..` or capital letters, or a block that is not there -> AC-007
- A settings zip that is missing, empty, cut short or refused -> AC-010
- An empty, half-written, folder or newer-format waiting file -> AC-010
- The disk fails while the zip is saved -> AC-011
- A block file whose `on` is not true or false, so the core refuses the zip -> AC-010
- Two commands writing the settings zip at the same moment -> out-of-scope: no locking between terminals yet; the agent process becomes the only writer in a later phase

## Out of scope
- Switching a routine on or off, and switching on several blocks at once
- Making `switch-on` clear a parked switch-off by itself: the user has `leko cancel`, and one command writes one file
- Any other change to a block (edit, remove, move): each is its own later contract
- A parked switch-on, a `--zone` or `--json` flag: switching on needs no moment and shows none
- Showing in `status` that a block was switched on or off by hand: status already reads the `on` field and shows the result
- Syncing the change to the phone, and landing it from a background process: a later phase

## Decisions
- `switch-on` is its own command, `leko switch-on FILE --id ID [--write]`, with no `--zone`, no `--json` and no clock | the name mirrors `switch-off` and `cancel`, and it needs no moment because nothing waits | REVERSIBLE
- Switching on is a tightening: it never waits, never needs a partner key, and the settings are read only to find the block and write it back | a user must always be able to protect themselves again at once; a core test pins that the loosening judge agrees, so a change to the table cannot silently make it wait | REVERSIBLE
- It writes `"on": true` into the block's file, even when the field was absent before | an explicit field is what a later reader and a diff both see; removing the field would change the file's shape | REVERSIBLE
- A parked switch-off of the same block stops the switch-on (exit 1) and points to `leko cancel` | one command writes one file, and `settle` could otherwise switch the block off again later | REVERSIBLE
- A bad waiting file stops the switch-on (exit 1), as it does for `switch-off` | without reading it, a parked loosening could be missed; the file is reported and left alone | REVERSIBLE
- New code goes in `core/src/switch_on/` (plan_switch_on, block_on_zip) and `cli/src/switch_on/` (arguments, report, run); the zip reader, the waiting-file reader and the atomic save are reused | folders match the feature, and `core/src/delayed_changes/` and `cli/src/delayed_changes/` already hold 5 and 6 files | REVERSIBLE
