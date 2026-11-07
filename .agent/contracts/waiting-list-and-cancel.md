# Contract: waiting-list-and-cancel

Written before any code. After the user opens it, the contract is locked: criteria cannot be edited to match the code.
Every "<...>" part must be replaced with real text. `/contract open <feature>` refuses a contract that is not complete.

## Goal
The user can see which parked changes touch an app or a site by asking `leko status`, and can take a parked change back with a new command, `leko cancel`. Taking a change back is a tightening, so it needs no wait and no key, and it only ever edits the waiting file, never the settings zip.

## Domains
Domains: cli, data

## Acceptance criteria
- AC-001: The core takes the parked changes out of a list by block id and nothing else: `cancel_waiting(changes, block)` returns the list without that block's record, every other record exactly as it was and in the same order, and says whether a record was removed or none was there; an id that is only a prefix, a different case or a path-shaped text matches nothing.
  - evidence: core/tests/cancel_waiting.rs "cancelling_removes_only_that_blocks_record_and_keeps_the_rest_in_order"
  - breaker: Every record goes, a prefix such as `a` removes `ab`, the order or a moment of another record changes, or a missing record is reported as removed.
- AC-002: The core picks, from the parked changes, the ones that belong to an app or a site: `waiting_for_target(set, target, changes, now, zone)` returns the settle step (the same outcome `leko settle` would give) for each record whose block covers the target, in file order; records of blocks that do not cover it, and of blocks no longer in the settings, are left out; a block that covers the target but is not closing it at this moment still counts.
  - evidence: core/tests/waiting_for_target.rs "only_changes_of_blocks_that_cover_the_target_come_back_with_what_settle_would_say"
  - breaker: Another block's change is shown, a covering block that is outside its hours is hidden, a removed block's record is shown for every target, or status says "due" where settle says "not yet" because it judged the lock a second way.
- AC-003: `leko cancel FILE --id ID` prints one plain draft and writes nothing (settings zip, waiting file, folder and home folder as they were), exit 0: it says the waiting change for the block would be cancelled, ends with "Nothing was saved" and the same command with `--write`.
  - evidence: cli/tests/cancel_draft.rs "without_the_write_flag_the_draft_says_what_would_be_cancelled_and_saves_nothing"
  - breaker: The draft already removes the record, leaves a temporary file, or does not say which block it is about.
- AC-004: With `--write` only the record of that block leaves `FILE.waiting.json`: the other records are byte for byte as they were in the same order, the list is kept (empty, if it was the last), the settings zip is byte for byte the same, the sentence says "Cancelled", and the saved waiting file passes the core's reader.
  - evidence: cli/tests/cancel_write.rs "write_removes_only_that_record_and_never_touches_the_settings_zip"
  - breaker: Another block's record goes with it, the waiting file is deleted when the list empties, the zip is rewritten, or the saved file is one the reader refuses.
- AC-005: Running it again, or on nothing, is safe and says what it did: with no waiting file, an empty list, or no record for the id, `cancel` exits 0 with "Nothing is waiting for the block ..." and "Nothing was saved", makes no file and changes no file; the same `cancel --write` typed twice leaves the second run a no-op; an id with `/`, `..` or capital letters is just a block nobody parked and never builds a path.
  - evidence: cli/tests/cancel_repeat.rs "a_second_cancel_or_a_cancel_of_nothing_changes_nothing_and_says_why"
  - breaker: A run on nothing makes an empty waiting file, a second run errors, a path-shaped id reaches the file system, or success is reported when nothing was removed.
- AC-006: Cancelling needs no wait, no key and no look at the settings: it works at once for a block whose lock is a long wait, a seal or a partner key, for a block that is no longer in the settings, and when the settings zip is missing or not a zip, because it reads and saves only the waiting file.
  - evidence: cli/tests/cancel_free.rs "a_cancel_is_never_held_by_a_lock_and_never_reads_the_settings_zip"
  - breaker: A cancel is made to wait or refused by the block's lock, a record of a removed block can never be cleared by hand, or a damaged settings zip stops the user from taking a request back.
- AC-007: Wrong or missing arguments print the usage line (which now lists `leko cancel`) to stderr and exit 2: no file, no `--id`, `--id` twice, `--id` with nothing after it, an unknown flag (including `--zone`, which cancel has no use for), a second file; `--write` given twice is also refused.
  - evidence: cli/tests/cancel_arguments.rs "bad_arguments_print_usage_and_exit_two"
  - breaker: A missing `--id` cancels the first record, an unknown flag is ignored, usage goes to stdout, or the usage line forgets the new command.
- AC-008: A waiting file that is empty, not JSON, not text, of a newer format number, holds a record the reader cannot read, is a folder, or is over 1 MB gives one plain sentence on stderr and exit 1 for `cancel`; it is never replaced or repaired, and a write on a file this version cannot read leaves it byte for byte as it was.
  - evidence: cli/tests/cancel_files.rs "a_bad_waiting_file_is_reported_and_left_alone_by_cancel"
  - breaker: A newer file is rewritten as format 1 and loses its extra data, a corrupt file is replaced by a clean one (the parked changes are lost), an empty file is read as "nothing waiting", or the sentence is a stack trace.
- AC-009: Every save of `cancel` goes through the temporary-file-then-move step used by the other commands: a failure at each step (write, flush, move) leaves the waiting file exactly as it was, leaves no temporary file behind, and exits 1 with one sentence naming the step.
  - evidence: cli/tests/cancel_atomic_save.rs "a_failure_at_any_step_leaves_the_waiting_file_as_it_was"
  - breaker: A failed save truncates the waiting file, leaves a half-written one or a temporary file, or reports success.
- AC-010: `leko status` reads the waiting file beside the settings file and, for each parked change of a block that covers the app or site asked about, adds one line before "Nothing was saved or changed." in the same words `leko settle` uses (block, the local moment it may land in the chosen zone, why it waits); when nothing applies, or there is no waiting file, the sentence is byte for byte what status said before this contract; status writes nothing and makes no file.
  - evidence: cli/tests/status_waiting.rs "status_lists_the_waiting_changes_of_the_blocks_that_cover_the_target"
  - breaker: The wording differs from settle's, another block's change is listed, an answer with nothing waiting changes, a missing waiting file is created, or the moment is shown in the wrong zone.
- AC-011: `leko status --json` always carries a `"waiting"` list: one object per applying change with the block id, the kind, the outcome word (`not-yet`, `due`, or one of the stopped or cleared words settle uses) and the UTC moment it may land (`null` when there is none); an empty list when nothing applies, and `null` when the waiting file could not be read.
  - evidence: cli/tests/status_waiting_json.rs "the_json_line_always_has_a_waiting_list_in_utc"
  - breaker: The key is missing when nothing waits (a script cannot tell "none" from "old program"), a moment is in local time, or an unreadable file shows as an empty list.
- AC-012: When the waiting file cannot be read, `leko status` still gives its answer on stdout with exit 0, adds the one-sentence problem on stderr, and does not touch the file; a file that is not there is not a problem and says nothing.
  - evidence: cli/tests/status_waiting_file.rs "status_still_answers_when_the_waiting_file_is_bad_and_says_so_on_stderr"
  - breaker: A corrupt sidecar takes the whole status down, the problem is swallowed, the problem goes to stdout where a script would read it as part of the answer, or a missing file is reported as an error.
- AC-013: The real `leko` program, with the real clock and zone, does the whole path on built zips: park a locked block's switch-off, see it in `status` for an app and a site that the block covers, `cancel --write` it, see that `status` no longer shows it and `settle` says nothing is waiting, then ask again and get a new request moment; results go to stdout, errors to stderr, and exit codes are 0 for done or nothing to do, 1 for refused, 2 for not understood.
  - evidence: cli/tests/real_cancel.rs "the_real_program_parks_shows_cancels_and_starts_again"
  - breaker: The built program differs from the tested functions (zone or clock not passed, command not wired), an error goes to stdout, a cancelled change keeps its old request moment when asked again, or an exit code does not say which kind of result it was.

## Checklist coverage
- CLI-1 -> criterion:AC-007
- CLI-2 -> criterion:AC-013
- CLI-3 -> criterion:AC-005
- DATA-1 -> criterion:AC-009
- DATA-2 -> criterion:AC-008
- DATA-3 -> criterion:AC-008

## Derived checklist
Not used: Domains is not "none".

## Edge cases
- Three parked changes and the middle one is cancelled -> AC-004
- Two blocks whose ids start the same (`a` and `ab`) -> AC-001
- An id with `/`, `..` or capital letters -> AC-005
- A parked change for a block that has since been removed -> AC-006
- The settings zip is missing, empty or not a zip when cancelling -> AC-006
- An empty, half-written, folder or newer-format waiting file -> AC-008
- The disk fails while the waiting file is saved -> AC-009
- An app that no block covers, and a block that covers the target but is outside its hours -> AC-002
- A bad waiting file when asking `status` -> AC-012
- The change is cancelled and asked for again -> AC-013
- Two commands writing the waiting file at the same moment -> out-of-scope: no locking between terminals yet; the agent process becomes the only writer in a later phase
- A change that falls due in the hour the clocks go back -> out-of-scope: the moment comes from the existing `due` function, which has its own daylight-time cases

## Out of scope
- Switching a block back on, editing or removing a block: each is its own later contract
- Cancelling every parked change at once, and a cancel that matches more than one block: one block per command for now
- A separate list command: `leko settle` without `--write` already lists every parked change, and `status` now shows the ones that touch what you ask about
- A change log or an undo of a cancel: the way back is to ask again, which starts the wait again
- Showing parked changes for the apps that are never blocked: that list is not applied yet
- Syncing the waiting list to the phone and landing changes in the background: the agent process, a later phase

## Decisions
- `cancel` is a command of its own, `leko cancel FILE --id ID [--write]`, not a flag of `switch-off` | the name says what it does, and `switch-on` will be a different command | REVERSIBLE
- Cancelling is a tightening: no wait, no partner key, no judge, and the settings zip is never opened | a request is only a note in the waiting file, and the user must always be able to take their own request back | REVERSIBLE
- `status` shows only the parked changes of blocks that cover the asked app or site, using the same settle plan and the same words as `leko settle` | one judgement and one wording for "when may this land"; the whole list stays in `settle` | REVERSIBLE
- With a bad waiting file `status` still answers (exit 0) and puts the problem on stderr | a status question has an answer even when the sidecar is damaged, and the problem is not swallowed | REVERSIBLE
- The JSON line of `status` always has a `waiting` key, so the two existing JSON expectations in the status tests gain it | a script can tell "nothing waits" from "an older program" | REVERSIBLE
- `cancel` has no `--zone` and no `--json` | it shows no moment, and `switch-off` and `settle` have no JSON either | REVERSIBLE
- New code goes in `core/src/delayed_changes/` (cancel_waiting, waiting_for_target: 7 files), `cli/src/cancel_change/` (cancel arguments, report, run) and `cli/src/status/` (status_waiting); the save step stays in `cli/src/add_block/file_replace.rs` and the file reader in `cli/src/delayed_changes/waiting_file_disk.rs`, both reused | folders match features, and `cli/src/delayed_changes/` already holds 6 files so a third and fourth file there would cross the 8-file rule | REVERSIBLE
