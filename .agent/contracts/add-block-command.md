# Contract: add-block-command

Written before any code. After the user opens it, the contract is locked: criteria cannot be edited to match the code.
Every "<...>" part must be replaced with real text. `/contract open <feature>` refuses a contract that is not complete.

## Goal
The user can type `leko add-block SETTINGS.zip --name NAME --app ID (or --site HOST) RULE` and first see a plain draft of the new block, then run the same words with `--write` to save it into the settings file in one safe step. This is the first command that changes a settings file, and it only adds: it never edits, delays or removes a block.

## Domains
Domains: cli, data

## Acceptance criteria
- AC-001: `leko add-block FILE --name NAME` with at least one `--app ID` or `--site HOST` and exactly one rule (`--always`, `--open-hours`, or `--for LENGTH`) reads FILE and prints one plain draft: the block's name and id, what it covers, the rule in words, the door, that never-blocked apps are not checked yet, "Nothing was saved", and the same command with `--write`; it writes nothing (the file's bytes, its folder and the home folder are as they were) and exits 0.
  - evidence: cli/tests/add_block_draft.rs "without_the_write_flag_the_draft_is_shown_and_nothing_is_saved"
  - breaker: The draft saves anyway, leaves a temporary file, changes the file's bytes, prints to stderr, or leaves out what the block covers or when it closes.
- AC-002: With `--write` the block shown in the draft is saved as `blocks/ID.json` in FILE and the sentence says "Saved"; every other file inside the zip keeps its name and content (tested on the real phone-exported files in `vectors/` as well as built ones), and the saved zip passes the core's check; a zip with no blocks, or only switched-off blocks, takes its first block.
  - evidence: cli/tests/add_block_write.rs "write_saves_the_drafted_block_and_leaves_every_other_file_as_it_was"
  - breaker: Another file changes or is lost, the saved block differs from the drafted one, `general.json` is rewritten, or the saved file fails the core's check.
- AC-003: The core builds the block from the request alone (name, apps, sites, rule, door, message, the moment): the same request gives byte-identical block JSON; the id comes from the name (ASCII letters and digits lowercased, every run of anything else one dash, dashes trimmed from both ends, at most 64 characters); a name that is empty, over 60 characters, holds a control character, or has no ASCII letter or digit is refused with a sentence and no block.
  - evidence: core/tests/block_draft.rs "the_same_request_makes_the_same_block_and_the_id_comes_from_the_name"
  - breaker: The id differs between runs, a name makes an id the readers refuse, a control character reaches the file, or an empty id is written.
- AC-004: Each rule kind means what the draft says, checked by reading the saved block back through the core and asking its decision at moments inside and outside: `--always` (optionally `--days mon,tue,...`) closes the target on those days all day; `--open-hours 09:00-17:00[,19:00-22:00]` (optionally with `--days`) leaves the target open only inside the stretches and closes it at all other times, including a stretch ending at `24:00` and one crossing midnight; `--for 90m` (any length `leko session` accepts) closes it until a session end that is the moment plus the length, with the matching seal.
  - evidence: core/tests/block_draft_rules.rs "each_rule_kind_reads_back_and_decides_as_the_words_say"
  - breaker: Hours are inverted (closed inside instead of outside), a day is dropped or misread, `24:00` is lost, the session ends at another moment than asked, or the seal and the session end differ.
- AC-005: The door and message follow fixed defaults: with no door flag the door is a wait of 30 seconds; `--wall` is no way in; `--wait N` is a wait of N seconds, 0 to 600; a `--for` block always has a wall door and refuses `--wait`; `--message TEXT` (1 to 300 characters, no control characters) is saved as the block's message; every new block is switched on.
  - evidence: cli/tests/add_block_door.rs "the_door_defaults_to_a_30_second_wait_and_a_timed_block_has_no_way_in"
  - breaker: A timed block gets a wait door that lets it out early, the default door is a plain button, a 601-second wait or a 301-character message is saved, or a new block is saved switched off.
- AC-006: A block is never overwritten: when FILE already holds a block with the same id (switched on or off, and also when two different names make one id), the command exits 1 with a sentence naming the id and saves nothing; running the same `--write` command twice saves once and the second run leaves the file exactly as the first left it.
  - evidence: cli/tests/add_block_repeat.rs "an_existing_block_is_never_overwritten_and_a_second_write_changes_nothing"
  - breaker: A second write replaces or duplicates a block, a switched-off block of the same id is replaced, or two names end up sharing one block file.
- AC-007: The new file is checked by the core before anything is saved, and the draft too: an app or site the core's readers refuse (an empty or 256-character package name, a name with spaces, a malformed site) or a result the zip rules would refuse (a file that already holds the most files a zip may hold) exits 1 with the core's own sentence, shows no draft, and saves nothing.
  - evidence: cli/tests/add_block_refused.rs "a_block_the_core_refuses_is_neither_shown_as_good_nor_saved"
  - breaker: A bad package name is saved, the draft looks fine but `--write` then fails, or the sentence is the program's guess instead of the core's.
- AC-008: The draft says when the phone's current Enforcer (0.3) could not read the saved file: the warning appears exactly when reading the result in the phone's mode is refused (a block with sites or a session rule), with the core's reason, and does not appear for a plain app block with an always or open-hours rule; the block is still drafted and, with `--write`, saved.
  - evidence: cli/tests/add_block_phone.rs "the_warning_appears_exactly_when_the_phone_would_refuse_the_file"
  - breaker: A file the phone refuses is saved with no word, or a plain app block gets a warning it should not.
- AC-009: Wrong or missing arguments (no command word, no file, two files, no `--name`, no app and no site, no rule or two rules, `--days` with `--for`, a bad day name, a bad `HH:MM-HH:MM`, `--wait` and `--wall` together, an app or site named twice, `--name` or another one-value flag given twice, a flag missing its value, an unknown flag, an argument that is not valid text) print the usage and one plain sentence on stderr, print nothing on stdout, read no file, and exit 2; `--help` names `add-block` in the usage and exits 0.
  - evidence: cli/tests/add_block_arguments.rs "wrong_or_missing_add_block_arguments_print_usage_on_stderr_and_exit_2"
  - breaker: A bad argument is guessed at and a block is still drafted, two rules silently pick one, usage goes to stdout, the exit code is 0 or 1, or a non-text argument crashes the program.
- AC-010: A file that cannot be added to is reported plainly and left alone: a missing path, a folder, an unreadable file, an empty file, a file that is not a zip, a zip the core refuses (the core's sentence is shown, and nothing is repaired or rewritten), and a file over 64 MB (refused by its size without being read) each exit 1 with one sentence on stderr, nothing on stdout, and the file and its folder unchanged, with or without `--write`.
  - evidence: cli/tests/add_block_file.rs "a_file_that_cannot_be_added_to_is_reported_and_left_untouched"
  - breaker: A corrupt file is overwritten by a fresh one, a refused file is half-read and a block still saved, a 1 GB file is read whole, or an error sentence reaches stdout.
- AC-011: The save is all-or-nothing: the new zip is written to a temporary file in the same folder, flushed, and moved over FILE in one step; if the temporary file cannot be made (read-only folder), the write fails part-way, or the move fails, FILE is byte-for-byte as it was, no temporary file is left, and the command exits 1 with a sentence; a good save leaves no temporary file, keeps the file's permission bits, and when FILE is a link replaces the file it points to and keeps the link.
  - evidence: cli/tests/add_block_replace.rs "a_failed_save_leaves_the_old_file_and_no_temp_file_and_a_good_save_keeps_mode_and_link"
  - breaker: A half-written zip replaces the settings, temporary files pile up, the link is replaced by a plain file, or the permissions change.
- AC-012: The core writes a zip from a list of files (names and bytes) that the core's reader and an independent tool (`unzip -t`, skipped when that program is missing) both read back with the same names and contents, including an empty file, non-ASCII names and a file of about 1 MB; it refuses, rather than write, a list the core's own reader would refuse (too many files, a file or a total over the reader's limits).
  - evidence: core/tests/zip_write.rs "written_zips_read_back_the_same_in_the_core_and_in_unzip"
  - breaker: A wrong size or checksum, a name in the wrong encoding, a zip only our own reader accepts, or a zip bigger than the reader's limits.
- AC-013: Results go to stdout and errors to stderr, never both on one run; exit 0 is a shown draft or a saved block, 1 a request understood but not possible (file, core refusal, existing id, failed save), 2 arguments not understood; a stdout that cannot be written (closed pipe) ends with exit 1 and no crash message, and when that happens after a `--write` the block is still saved in full.
  - evidence: cli/tests/real_add_block.rs "the_real_program_keeps_streams_and_codes_apart_and_survives_a_closed_pipe"
  - breaker: An error sentence reaches stdout, a refusal exits 0, a closed pipe prints a panic, or a closed pipe after the save leaves a half-saved file.
- AC-014: The clock is read in one place only, `cli/src/main.rs`, which stays at 40 lines or fewer; the real program ends a `--for` block at the real time it ran plus the length (within a minute), and the new files get the moment as an argument.
  - evidence: cli/tests/real_add_block.rs "a_timed_block_ends_the_asked_length_after_the_real_clock"
  - breaker: The session end is a built-in or remembered moment, or an add-block file reads the clock itself (`cli/tests/clock_at_the_edge.rs` scans every file under `cli/src` for that), so a test can no longer fix the moment.

## Checklist coverage
- CLI-1 -> criterion:AC-009
- CLI-2 -> criterion:AC-013
- CLI-3 -> criterion:AC-006
- DATA-1 -> criterion:AC-011
- DATA-2 -> criterion:AC-010
- DATA-3 -> criterion:AC-002

## Derived checklist
Not used: Domains is cli, data, which have their own checklists above.

## Edge cases
- A settings zip with no blocks, or only switched-off blocks -> AC-002
- A name that is empty, `!!!`, `Café`, 61 characters, or holds a newline or an escape character -> AC-003
- `Social Media` and `social-media`, which make one id -> AC-006
- Hours ending at `24:00`, a stretch crossing midnight, two stretches that overlap, and equal start and end -> AC-004
- An app name that is empty, 256 characters, or has a space; a site that is not a host -> AC-007
- The output folder is read-only, the disk fills part-way through, or the move over the old file is refused -> AC-011
- FILE is a link, or has permission bits other than the default -> AC-011
- stdout closed before the program writes (`| head -c 0`), including after a `--write` -> AC-013
- A zip already holding the most files a zip may hold -> AC-007
- The settings file is changed by the phone's sync between the read and the move -> out-of-scope: the command reads the file once and swaps once; guarding against a writer outside this program needs file locking, which belongs with the later sync and queue contracts

## Out of scope
- Editing, delaying, switching off or removing a block, and the list of waiting changes in `status` (the next Phase 3 contracts); adding a block only tightens, so the loosening judge is not needed here.
- Making a settings file when none exists, and finding where the app keeps it (FILE is always handed in).
- A request in free words, voice, and the local model (Phase 4); this command takes flags.
- Rule kinds `minutes` and `opens`, passes, earn, locks other than the session seal, and doors other than wait and wall; covering "all apps except".
- Looking up the app's bundle id by name, matching phone package names to Mac ids, and the never-blocked list.
- Closing or opening anything on the computer (the decision loop and the macOS ports).
- JSON output for the draft, Windows, packaging, and shell completion.

## Decisions
- The request is flags (`--name`, `--app`, `--site`, `--always`, `--open-hours`, `--for`, `--days`, `--wall`, `--wait`, `--message`), not free words | the same fields are what the later voice layer and the local model will fill in, so one core function serves all three | REVERSIBLE
- The confirm step is running the same command again with `--write`; there is no prompt on stdin | a script, a test and a voice layer can all drive it, and the same words always give the same draft | REVERSIBLE
- Only new blocks are added, and an existing id is refused | adding a block only tightens, so no loosening judge is involved; edit, delay and remove each need their own rules | REVERSIBLE
- The id is made from the name by a fixed rule, with no `--id` flag | the user types one thing, and the draft shows the id before anything is saved | REVERSIBLE
- With no door flag the door is a wait of 30 seconds, the mock-up's default; a `--for` block has no way in | a session must hold until it ends (the earlier "no way in during a session" decision) | REVERSIBLE
- Hours are the stretches when the target is open, closed at all other times, on the computer's clock; no zone is stored in the file | this is how the phone's outside-hours rule already reads, and `status` supplies the zone when it judges | REVERSIBLE
- The draft is checked in the extended mode (an error if refused) and in the phone's mode (a warning if refused) | the phone's 0.3 refuses sites and sessions, and the user should know before saving | REVERSIBLE
- The zip is rewritten whole by a new pure writer in the core, and the command-line crate does the file work (temporary file, flush, move) | the core makes no calls to the operating system | REVERSIBLE
- The size check against the 64 MB limit reuses the status command's file reader | one place holds the limit | REVERSIBLE
- New code goes in `core/src/blocks/`, `core/src/io/zip_writer.rs` and `cli/src/add_block/` | the folders match the feature, beside `decision/` and `status/` | REVERSIBLE
