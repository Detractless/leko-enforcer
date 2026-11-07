# Contract: delay-block-change

Written before any code. After the user opens it, the contract is locked: criteria cannot be edited to match the code.
Every "<...>" part must be replaced with real text. `/contract open <feature>` refuses a contract that is not complete.

## Goal
The user can switch a block off from the terminal. If nothing locks the block, it goes off at once; if a lock makes the change wait, the change is parked in a plain waiting file next to the settings file with the moment it may land, and a second command, `leko settle`, lands every parked change that is due. This is the first loosening change the tool makes, so it goes through the loosening judge and the locks, never around them.

## Domains
Domains: cli, data

## Acceptance criteria
- AC-001: The core decides a switch-off from the settings, one block id, the moment and the zone alone (no clock, no files): it sends the change through the loosening judge (`diff_item` says the `on` unit loosens), takes the locks that apply, and answers exactly one of: lands now, waits until a moment, cannot land by itself, needs a partner key, already off, or no such block. For the block's own lock: open lands now; a wait of 24 hours lands 24 hours later; change hours land inside the window after their hours; a seal until a date lands at the first midnight after it; a seal at an exact moment lands at that moment; a seal that has already ended lands now; change windows that share no minute answer "cannot land by itself".
  - evidence: core/tests/switch_off_plan.rs "a_switch_off_is_judged_and_each_kind_of_lock_gives_its_moment"
  - breaker: The judge is skipped, a wait is counted from the wrong moment, a seal that ended still delays, a window with no shared minute lands anyway, or a block with no `on` field (on by default) is called already off.
- AC-002: The locks that apply to switching a block off are the block's own lock and the lock of every routine that holds the block (Enforcer's guard rule); the change lands at the latest of their moments, a routine that does not hold the block adds nothing, and any applied lock that carries a partner key makes the answer "needs a partner key".
  - evidence: core/tests/switch_off_plan.rs "routines_that_hold_the_block_add_their_locks_and_a_partner_key_stops_the_change"
  - breaker: A routine's lock is ignored (the block goes off through a side door), an unrelated routine delays the change, the earliest moment is used instead of the latest, or a partner key is skipped.
- AC-003: `leko switch-off FILE --id ID` prints one plain draft and writes nothing (file bytes, folder and home folder as they were), exit 0: for a block that lands now it says the block would be switched off now; for a block that must wait it says the change would be parked, the local date and time it may land in the chosen zone, and which lock makes it wait; both end with "Nothing was saved" and the same command with `--write`.
  - evidence: cli/tests/switch_off_draft.rs "without_the_write_flag_the_draft_says_when_it_lands_and_saves_nothing"
  - breaker: The draft saves or leaves a temporary file, shows a moment in the wrong zone, hides the lock that causes the wait, or tells the user the block is already off when it is only parked.
- AC-004: With `--write` and a change that lands now, the block's file in the zip gets `"on": false` and nothing else in the block changes; every other file in the zip keeps its name and content (tested on the real phone-exported files in `vectors/` and on built zips); the sentence says "Saved"; the saved zip passes the core's check.
  - evidence: cli/tests/switch_off_write.rs "write_switches_the_block_off_and_leaves_every_other_file_as_it_was"
  - breaker: Another file or another field of the block changes, the block is removed instead of switched off, the zip is rewritten when nothing needed changing, or the saved zip fails the check.
- AC-005: With `--write` and a change that must wait, only the waiting file `FILE.waiting.json` (the settings file's name plus `.waiting.json`, in the same folder) is written, holding the block id, the moment of the request and the moment it may land; the settings zip is byte for byte the same; the sentence says "Waiting" and the local moment.
  - evidence: cli/tests/switch_off_park.rs "write_parks_a_locked_change_and_the_settings_zip_is_not_touched"
  - breaker: A locked block is switched off at once, the zip is touched, the waiting file lands in another folder, or the due moment is saved in the wrong zone or unit.
- AC-006: Running a command twice is safe and says what it did: `switch-off` on a block that is already off exits 0 with "already off" and saves nothing; on a block that is already waiting it exits 0 saying it is waiting and until when, and the waiting file is left exactly as it was (no second record, same request moment).
  - evidence: cli/tests/switch_off_repeat.rs "a_second_switch_off_changes_nothing_and_says_why"
  - breaker: A second run adds a second record, moves the request moment later (which would restart the wait), or reports success as if it had saved.
- AC-007: A request the core cannot honour exits 1 with one line ending ". Nothing was saved." and saves nothing: an id that no block has (an id with a path separator or `..` is just unknown and never builds a path), a block whose applied lock has a partner key, and change windows that share no minute.
  - evidence: cli/tests/switch_off_refused.rs "each_refusal_is_one_line_names_the_reason_and_saves_nothing"
  - breaker: A path-shaped id reaches the file system, a partner-key block is switched off, a "cannot land" change is parked forever without a word, or a refusal saves part of the change.
- AC-008: Wrong or missing arguments print a usage line to stderr and exit 2 for both commands: no file, no `--id`, `--id` twice, an unknown flag, a stray word, `settle` with `--id`, and a zone that does not exist; the zone is taken from `--zone`, else `TZ`, else `/etc/localtime`, and never guessed.
  - evidence: cli/tests/delay_arguments.rs "bad_arguments_print_usage_and_exit_two_for_both_commands"
  - breaker: A missing `--id` switches off the first block, an unknown flag is ignored, usage goes to stdout, or an unknown zone falls back to UTC.
- AC-009: A settings file that is missing, empty, not a zip, refused by the core, or over 64 MB, and a waiting file that is empty, not JSON, of an unknown format number, or holds a record the reader cannot read, each give one plain sentence on stderr and exit 1; no file is overwritten, and a bad waiting file is never replaced by a new one.
  - evidence: cli/tests/delay_files.rs "a_bad_settings_file_or_waiting_file_is_reported_and_nothing_is_overwritten"
  - breaker: A corrupt waiting file is silently replaced (losing parked changes), an empty file is read as "nothing waiting", a 5 GB file is read into memory, or the sentence is a stack trace.
- AC-010: Every save goes through the temporary-file-then-move step from `add-block`: a failure at each step (create, write, flush, move) leaves the target exactly as it was and no temporary file behind, for the settings zip and for the waiting file.
  - evidence: cli/tests/delay_atomic_save.rs "a_failure_at_any_step_leaves_both_files_as_they_were"
  - breaker: A failed save truncates the target, leaves a half-written waiting file, or leaves a temporary file in the folder.
- AC-011: The waiting file is `{"format": 1, "changes": [...]}` with one record per change (kind, block id, request moment, due moment, both in UTC text); a file this version wrote reads back to the same records; a file with a later format number, a missing field or an unknown field is refused whole with a sentence and left alone.
  - evidence: core/tests/waiting_file.rs "a_waiting_file_reads_back_the_same_and_a_newer_or_odd_one_is_refused"
  - breaker: A newer file is read as if it were format 1 and rewritten without its extra data, a record with a missing moment is read as "due now", or the round trip changes a moment.
- AC-012: `leko settle FILE` with no waiting file, or an empty list, says "Nothing is waiting", exits 0 and creates no file; with waiting changes that are not yet due it lists each (block, due local moment, why) and changes nothing, even with `--write`; a moment earlier than the request moment (the clock was set back) counts as not due.
  - evidence: cli/tests/settle_waiting.rs "settle_lists_what_is_not_due_and_changes_nothing"
  - breaker: An empty run creates an empty waiting file, a change lands early, a clock set back lands everything, or the list leaves out why it waits.
- AC-013: `leko settle FILE` shows a draft of the due changes and saves nothing; with `--write` each due change switches its block off in the zip (as in AC-004) and its record is removed from the waiting file (the list is kept, empty if it was the last); changes that are not yet due stay in the file untouched; the zip is saved first, the waiting file second.
  - evidence: cli/tests/settle_land.rs "write_lands_only_the_due_changes_and_keeps_the_rest_waiting"
  - breaker: A change that is not due lands with the due ones, a landed record stays and lands again, the waiting file is saved before the zip (a crash loses the change), or the draft saves.
- AC-014: Settle checks the block as it is now, not as it was when the change was parked: it never lands earlier than the later of the saved due moment and the moment the block's present locks give for the original request moment (a lock tightened since the request pushes it later); a block that has been removed or is already off has its record cleared with a sentence; a partner key added since stops the landing and keeps the record, with a sentence.
  - evidence: cli/tests/settle_recheck.rs "settle_judges_the_block_as_it_is_now_and_never_lands_early"
  - breaker: A lock tightened after the request is ignored, a hand-edited earlier due moment lands a locked block, a removed block leaves a record that can never clear, or a partner-key block lands.
- AC-015: If the zip is saved and the waiting file then cannot be updated, settle says so plainly and exits 1; running it again finishes cleanly (the block is already off, so the record is cleared) without switching anything twice.
  - evidence: cli/tests/settle_halfway.rs "a_failed_second_save_is_reported_and_a_rerun_finishes_the_job"
  - breaker: The failure is reported as success, the record stays forever, or the rerun errors on "already off" and leaves the record.
- AC-016: The real `leko` program, run with the real clock and zone on built zips and on the real phone-exported zips in `vectors/`, does the whole path: switch off an unlocked block, park a locked one, list it, and land it once the lock has run out (a wait of 0 hours and a seal that ended give due changes at once); results go to stdout, errors to stderr, and the exit codes are 0 for done or nothing to do, 1 for refused, 2 for not understood.
  - evidence: cli/tests/real_delay.rs "the_real_program_switches_off_parks_lists_and_lands"
  - breaker: The built program differs from the tested functions (zone read wrongly, clock not passed), an error goes to stdout, or the exit code does not say which kind of result it was.

## Checklist coverage
- CLI-1 -> criterion:AC-008
- CLI-2 -> criterion:AC-016
- CLI-3 -> criterion:AC-006
- DATA-1 -> criterion:AC-010
- DATA-2 -> criterion:AC-009
- DATA-3 -> criterion:AC-011

## Derived checklist
Not used: Domains is not "none".

## Edge cases
- A block with no `on` field (on by default) and no lock -> AC-001
- A block held by two routines, one with a 24-hour wait and one with a 72-hour wait -> AC-002
- A block named by an id with `/`, `..` or capital letters -> AC-007
- An empty settings file, and a waiting file that is empty or half-written -> AC-009
- The same `switch-off --write` typed twice -> AC-006
- The zip saved but the waiting file cannot be updated -> AC-015
- The clock set back between the request and `settle` -> AC-012
- A waiting record for a block that has since been removed -> AC-014
- A change that falls due in the hour the clocks go back -> out-of-scope: the due time comes from the existing `due` function, which already has its own daylight-time cases in `vectors/loosening-cases.json`
- Two commands writing the waiting file at the same moment -> out-of-scope: no locking between terminals yet; the agent process becomes the only writer in a later phase

## Out of scope
- Editing any other field of a block, removing a block, and delaying any other loosening: each is its own later contract
- Switching a block back on (a tightening that needs no wait) and cancelling a waiting change ("Undo"): the next contracts
- Listing waiting changes inside `leko status`: a later contract, which reads the file format fixed here
- A signed or tamper-proof queue, a change log, and unlocking with a partner's key: the waiting file is a plain file, so anyone who can edit the files can already edit the zip
- Landing a change by itself in the background and syncing the waiting list to the phone: the agent process, a later phase
- Emergency passes and routines' own `ifSkipped` rules

## Decisions
- The waiting file is `FILE.waiting.json` beside the settings zip, not a file inside the zip | a settings zip has no pending list in the phone's format, so a record inside it would be a field the phone refuses | REVERSIBLE
- A record stores the request (block id, request moment, due moment), not the finished block | settle judges the block as it is on the day it lands, so a lock tightened in between still counts | REVERSIBLE
- Landing is a command the user runs, `leko settle`, not something that happens in the background | the agent process that may land changes by itself belongs to a later phase | REVERSIBLE
- One contract holds both `switch-off` and `settle` | a parked change that nothing can land is half a feature, and the waiting file format needs both ends to be tested together | REVERSIBLE
- Locks that apply are the block's own and those of the routines that hold it, as Enforcer's `guards.js` does; the general lock is not read | this is the phone's rule for a block, and skipping the routines would be a side door | REVERSIBLE
- A lock with a partner key is refused, not bypassed | this tool cannot show a partner's key, and the refusal is the safe side | REVERSIBLE
- The zone comes from `--zone`, else `TZ`, else `/etc/localtime`, never guessed, like `status` | one rule for the whole program | REVERSIBLE
- `settle` saves the zip first and the waiting file second | a crash between them leaves a record for a block already off, which the next run clears, instead of losing the change | REVERSIBLE
- The draft-then-`--write` style is the same as `add-block` | one habit for every command that changes a file | REVERSIBLE
- New code goes in `core/src/waiting/` (waiting records and file, switch-off plan, settle plan), `cli/src/switch_off/` and `cli/src/settle/`; the shared save step stays in `cli/src/add_block/file_replace.rs` and is reused | folders match features, and no folder holds more than 8 code files | REVERSIBLE
