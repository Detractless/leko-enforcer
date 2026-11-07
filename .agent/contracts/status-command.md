# Contract: status-command

Written before any code. After the user opens it, the contract is locked: criteria cannot be edited to match the code.
Every "<...>" part must be replaced with real text. `/contract open <feature>` refuses a contract that is not complete.

## Goal
The user can type `leko status SETTINGS.zip --app NAME` (or `--site HOST`) and get a plain answer for that one app or site right now, on the real clock and the computer's time zone: closed or open, which blocks and rules close it, which rules that count use could not be judged, and until when the answer holds. It reads the one settings file and changes nothing.

## Domains
Domains: cli

## Acceptance criteria
- AC-001: `leko status FILE --app NAME` and `leko status FILE --site HOST` read the settings zip FILE and print one sentence saying whether that app or site is closed or open at the given moment, naming every closing block and rule (block id, rule id, kind) as the core's decision lists them, the moment and zone it was judged in, and "Nothing was saved or changed"; for an app it also says that never-blocked apps are not applied yet; exit 0 whether the answer is closed or open.
  - evidence: cli/tests/status_command.rs "an_app_or_a_site_is_reported_closed_with_its_closing_rules_or_open_when_none_closes"
  - breaker: A closing rule is left out, a block that does not cover the target is named, a site is judged as an app (or the reverse), an open answer exits non-zero, or the sentence is judged at another moment than the one handed in.
- AC-002: Rules that count use (minutes, opens) of blocks that cover the target are named in the sentence as "not judged" and are never called open or closed; a target covered only by such rules is reported open, with those rules named.
  - evidence: cli/tests/status_command.rs "rules_that_count_use_are_named_as_not_judged_and_never_called_closed_or_checked"
  - breaker: A minutes rule is reported as satisfied or as closing, is left out of the sentence, or makes the answer say the target is fully free.
- AC-003: Wrong or missing arguments (no command word, no file, two files, neither or both of `--app` and `--site`, an empty name or host, a flag given twice, a flag missing its value, an unknown flag, an argument that is not valid text) print the usage and one plain sentence on stderr, print nothing on stdout, read no file, and exit 2; `--help` prints the usage, now naming `status`, on stdout and exits 0.
  - evidence: cli/tests/status_arguments.rs "wrong_or_missing_status_arguments_print_usage_on_stderr_and_exit_2"
  - breaker: A bad argument is guessed at and answers anyway, `--app` and `--site` together pick one silently, usage goes to stdout, the exit code is 0 or 1, or a non-text argument crashes the program.
- AC-004: The wall clock the rules are judged on is the zone named by `--zone NAME` when given, otherwise the computer's zone handed in by `main.rs`; an unknown zone name gives exit 1 with the core's sentence, and no known computer zone with no `--zone` gives exit 1 with a sentence asking for `--zone`; in each refusal stdout stays empty and UTC is never assumed.
  - evidence: cli/tests/status_zone.rs "the_zone_flag_beats_the_computers_zone_and_an_unknown_or_missing_zone_gives_no_answer"
  - breaker: The zone flag is ignored, an outside-hours rule is judged in UTC, a misspelled zone is read as UTC, or a computer whose zone cannot be found silently answers in UTC.
- AC-005: The core writes a moment on a zone's wall clock as `YYYY-MM-DDTHH:MM:SS` plus the offset in force then (`-05:00`, `+05:30`, `Z` for a zero offset), correct on the repeated hour of autumn (`-04:00` then `-05:00` for the same wall minutes), across the skipped hour of spring, in half-hour zones, and with seconds kept; a moment the zone has no rule for is written in UTC with `Z`.
  - evidence: core/tests/local_time_text.rs "a_moment_is_written_on_the_wall_clock_with_the_offset_in_force_then"
  - breaker: The offset of another date is shown, the autumn hour looks the same both times, a half-hour zone loses its 30 minutes, or an out-of-range moment crashes.
- AC-006: The core gives the next flip of the answer for one target: the first moment after the given one at which closed or open differs for that app or host, looking 8 days ahead by the minute and at every session end, exactly that moment (a stretch edge is that minute, a session end is that second); a change in a block that does not cover the target, and a rule opening while another still closes, are not a flip; when nothing flips within 8 days, for a never-blocked app, and at the largest moment the core can count, the answer is none and nothing overflows.
  - evidence: core/tests/status_next_flip.rs "the_next_flip_is_the_first_moment_the_answer_for_that_target_differs"
  - breaker: An unrelated block's edge is reported for this app, a flip is missed or late, a rule opening under a still-closing rule is reported as the end of the closure, a never-blocked app gets a moment, or a far-away moment overflows.
- AC-007: The sentence says until when the answer holds, in the zone's own time with its offset: "closed until T", "open until T", or that it stays closed or open for at least the next 8 days; a session that ended a second ago is open and one ending in two days is closed until then.
  - evidence: cli/tests/status_next_change.rs "the_sentence_says_until_when_the_answer_holds_in_local_time"
  - breaker: The time is shown in UTC only or in the wrong zone, "until" is the next unrelated change, an open answer says closed until, or an answer that never changes promises a time.
- AC-008: With `--json` (before or after the other arguments), stdout is exactly one line: an object with the target (kind and name), the moment judged (UTC, `YYYY-MM-DDTHH:MM:SSZ`), the zone name, `closed`, `closing` and `not_judged` (each a list of block, rule and kind) and `until` (UTC or null), equal to what the core returns for the same inputs; without `--json` stdout is the sentence form only.
  - evidence: cli/tests/status_output.rs "json_is_one_line_holding_what_the_core_decides_for_the_same_inputs"
  - breaker: The JSON has a second line, differs from the core's decision, shows local time instead of UTC, or the sentence and the JSON disagree.
- AC-009: A file that cannot give an answer gives no answer: a missing path, a folder, an unreadable file, an empty file, a file that is not a zip, a zip the core refuses (the core's sentence is shown), and a file over 64 MB (refused without reading it) each exit 1 with one plain sentence on stderr and nothing on stdout; a settings zip with no blocks is a real answer (open), not an error.
  - evidence: cli/tests/status_file.rs "a_file_that_cannot_be_read_or_is_refused_gives_no_answer_and_exit_1"
  - breaker: A missing or corrupt file is reported as open, a refused file is half-read, a 1 GB file is read whole, or an error sentence reaches stdout.
- AC-010: Results go to stdout and errors to stderr, never both on one run; exit 0 is an answer (closed or open), 1 a request understood but not answerable (file, zone), 2 arguments not understood; running the real program twice gives the same answer twice and leaves the settings file (byte for byte), its folder and the home folder as they were; a stdout that cannot be written (closed pipe) ends with exit 1 and no crash message.
  - evidence: cli/tests/real_status.rs "status_changes_nothing_uses_the_streams_and_exit_codes_and_survives_a_closed_pipe"
  - breaker: The program writes a file or leaves a state behind, a refusal exits 0, an error sentence reaches stdout, the second run differs, or a closed pipe prints a panic.
- AC-011: The real program judges at the real clock and in the computer's zone: the moment it prints is the time it ran (within a minute), and the zone is `TZ` when that is set (a leading colon ignored), otherwise the zone `/etc/localtime` points to; an invalid `TZ` gives exit 1 and asks for `--zone`, and `--zone` beats `TZ`.
  - evidence: cli/tests/real_status.rs "the_real_program_judges_at_the_real_clock_in_the_computers_zone"
  - breaker: The moment is a built-in or remembered one, the zone is UTC whatever the computer says, `TZ` is ignored, or a bad `TZ` is read as UTC.
- AC-012: The clock and the computer's zone are read in one place only, `cli/src/main.rs`, which stays at 40 lines or fewer and calls the library; every other file of the command-line crate (the new status files included) gets the moment and the zone name as arguments.
  - evidence: cli/tests/clock_at_the_edge.rs "only_main_reads_the_clock"
  - breaker: A status file reads `TZ`, the clock or `/etc/localtime` itself, so a test can no longer fix them, or `main.rs` grows into a place where logic hides.

## Checklist coverage
- CLI-1 -> criterion:AC-003
- CLI-2 -> criterion:AC-010
- CLI-3 -> criterion:AC-010

## Derived checklist
Not used: Domains is cli, which has its own checklist above.

## Edge cases
- A settings zip with no blocks, only switched-off blocks, or no block that names the target -> AC-001
- An app name or host that is empty, 1 MB long, or holds control characters (shown safely, not printed raw) -> AC-003
- A session that ended one second ago, and one ending in two days -> AC-007
- An always rule on every day (never changes), and a rule that opens under another rule that still closes -> AC-006
- An earlier edge in a block that covers a different app -> AC-006
- The autumn repeated hour and a half-hour zone such as Asia/Kolkata in the printed time -> AC-005
- `--zone ""`, `--zone Mars/Base`, `TZ` set to nonsense, and no zone found at all -> AC-004
- The file is a folder, is missing, is empty, is 65 MB, or is a zip cut in half -> AC-009
- stdout closed before the program writes (`| head -c 0`) -> AC-010
- The clock set before 1970: main.rs turns the failed clock read into exit 1 with a sentence -> out-of-scope: a test cannot set the real clock; main.rs holds only this one branch and it is read in review, not tested
- The settings file is changed by the phone while the command reads it -> out-of-scope: the command reads the file once, whole; later steps own file watching

## Out of scope
- Closing or opening anything, watching in a loop, and waking at the next flip (the decision loop and the macOS ports).
- Whether the user may get through a closure: passes, doors, walls, routine holds and locks.
- Counting use for `minutes` and `opens` rules (needs the macOS usage reader); they are named as not judged.
- The real Mac never-blocked list, and matching Android package names to Mac bundle ids (the shared catalog naming); the sentence says never-blocked apps are not applied.
- Finding the settings file where the app stores it, listing the apps and sites a block covers, and asking about several targets in one run.
- Reading the computer's zone on Windows (Windows is a later phase).
- Languages other than English, and packaging, installing and shell completion.

## Decisions
- One target per run, `--app NAME` or `--site HOST`, and the settings zip as the one other argument | a short sentence a voice layer can read back; the later loop uses the core directly | REVERSIBLE
- The answer is judged at the moment handed in by `main.rs`, with no `--now` flag | tests fix the moment and a user cannot fake one | REVERSIBLE
- The zone is `--zone`, else `TZ`, else the `/etc/localtime` link, and never UTC by guess | a wrong zone changes the answer for every outside-hours rule | REVERSIBLE
- `leko_cli::run` gets the computer's zone name as one more argument, and the existing session tests are updated for it | the zone, like the clock, is read at the edge only | REVERSIBLE
- "Until" is the next flip of closed or open for that target, a new core function that does not change the existing `next_change` | a user asks "when does it open", not "when does anything in my settings change" | REVERSIBLE
- Times in the sentence are local with the offset, JSON times are UTC | readable for a person, stable for a script | REVERSIBLE
- Never-blocked apps are an empty set for now and the sentence for an app says so | the real list is a later contract; saying it avoids a wrong "closed" for Finder looking like a promise | REVERSIBLE
- Exit 0 for any answer, closed or open | open is not an error; scripts read `closed` in the JSON | REVERSIBLE
- The file is read whole after a size check against the core's 64 MB limit | the core reads a zip from memory, so a huge file must be refused first | REVERSIBLE
- The new code goes in `cli/src/status/` and `core/src/status/` | the folders match the feature, beside `session/` and `decision/` | REVERSIBLE
