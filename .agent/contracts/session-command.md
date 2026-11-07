# Contract: session-command

Written before any code. After the user opens it, the contract is locked: criteria cannot be edited to match the code.
Every "<...>" part must be replaced with real text. `/contract open <feature>` refuses a contract that is not complete.

## Goal
The user can type `leko session 30`, or hand `leko voice "start a 30 minute session"` the words a speech-to-text heard, and get a session rule and a moment seal that end together, counted from the real clock. Nothing is saved or changed yet (the result is shown, for the later steps to apply), and a spoken phrase that is unclear, or asks for the opposite, makes nothing.

## Domains
Domains: cli

## Acceptance criteria
- AC-001: `leko session 30` prints a sentence with the length in minutes, the end moment in UTC (YYYY-MM-DDTHH:MM:SSZ) and the words "Nothing was saved or changed", and exits 0; the end is the given current moment plus 30 minutes, a part of a second rounding up (10:00:00.400 plus 30 minutes ends 10:30:01Z).
  - evidence: cli/tests/session_command.rs "minutes_make_a_session_that_ends_that_long_after_the_given_moment"
  - breaker: The end is counted from another moment than the one handed in, minutes are taken for seconds or hours, or the sentence promises a change that did not happen.
- AC-002: A length is a whole number of minutes (`90`) or one whole number with `m`, `h` or `d` (`90m`, `2h`, `7d`), from 1 minute to 7 days (10080 minutes) after conversion; outside that, or too big to count, exits 1 with the core's own sentence on stderr and nothing on stdout.
  - evidence: cli/tests/session_lengths.rs "lengths_in_minutes_hours_and_days_stay_within_one_minute_to_seven_days"
  - breaker: `2h` makes 2 minutes, `8d` or `10081` is accepted, `0` makes a session that has already ended, or a 30-digit number or `99999999999999999h` crashes instead of being refused.
- AC-003: Wrong or missing arguments (no command, an unknown command or flag, no length, two lengths, a flag given twice, a flag missing its value, a length such as `1.5h`, `1h30m`, `-5` or `٣٠`, an argument that is not valid text) print a usage line and one plain sentence on stderr, print nothing on stdout, make nothing, and exit 2; `--help` prints the usage on stdout and exits 0.
  - evidence: cli/tests/session_arguments.rs "wrong_or_missing_arguments_print_usage_on_stderr_and_exit_2"
  - breaker: A bad argument is guessed at and makes a session, the usage goes to stdout, the exit code is 0 or 1, or a non-text argument crashes the program.
- AC-004: The rule is named `session` unless `--id` is given with a name (for example `--id focus`); a name that is not letters, digits, `-` or `_` (1 to 64 characters) exits 1 with the core's sentence and makes nothing.
  - evidence: cli/tests/session_arguments.rs "rule_id_defaults_to_session_and_bad_ids_are_refused"
  - breaker: A name with a slash, a space, 65 characters or nothing at all reaches the output.
- AC-005: With `--json` (before or after the length), stdout is exactly one line: an object with `rule` and `lock` that equal what the core's session maker returns for the same moment, length and name, and both end at the same moment; without `--json` stdout is the sentence form only.
  - evidence: cli/tests/session_output.rs "json_is_one_line_holding_the_rule_and_the_seal_the_core_makes"
  - breaker: The JSON has a second line, differs from the core's rule or seal, or the rule and the seal end at different moments.
- AC-006: `leko voice` followed by the heard words in quotes makes exactly the same session (same sentence, same JSON with `--json`) as `leko session` with the length it understood. It understands the word "session" with exactly one length, said as digits and a unit (minute, minutes, min, mins, hour, hours, hr, hrs, day, days), as `a`, `an` or `one` and a unit, or as "half an hour" or "half hour", in any case, with punctuation and a hyphen between number and unit ("25-minute") ignored.
  - evidence: cli/tests/voice_phrases.rs "spoken_phrases_make_the_same_session_as_the_typed_length"
  - breaker: "2 hours" makes 2 minutes, "half an hour" is not 30 minutes, "a day" is not 1440, or the phrase path ends at another moment than the typed path.
- AC-007: A phrase that is unclear or asks for the opposite makes nothing, exits 2, and says what it understood and the forms it takes: no "session", no length, two lengths ("10 or 20 minutes"), an unknown unit ("30 seconds", "3 weeks"), a number in words other than a, an or one ("twenty minutes"), zero, a minus or a decimal ("-5 minutes", "1.5 hours"), "half" in any other form ("an hour and a half"), a word that cancels or denies (cancel, stop, end, remove, delete, undo, unlock, shorten, skip, disable, quit, abort, no, not, dont, never, without), empty text, or more than 500 characters.
  - evidence: cli/tests/voice_phrases.rs "unclear_or_opposite_phrases_make_nothing"
  - breaker: "cancel my 30 minute session" or "don't start a 10 minute session" seals the user in, "an hour and a half" makes 60 minutes, or "20 or 30 minutes" picks one.
- AC-008: Running the real program twice with the same words makes two independent answers, each saying it saved nothing, and leaves the folder it ran in and the home folder exactly as they were; empty input (`leko voice ""`) says nothing was made and does not fall back to a default length.
  - evidence: cli/tests/real_program.rs "running_twice_or_on_empty_input_changes_nothing_and_says_so"
  - breaker: The program leaves a file or a state behind, the second run is refused or different in shape, or empty input makes a default session.
- AC-009: The real program puts results on stdout and errors on stderr, never both on one success or failure, with exit 0 for a session made, 1 for a request understood but refused (length or name), 2 for arguments or words not understood; a stdout that cannot be written (closed pipe) ends with exit 1 and no crash message.
  - evidence: cli/tests/real_program.rs "exit_codes_and_streams_tell_success_refusal_and_usage_apart"
  - breaker: An error sentence reaches stdout, a refusal exits 0, a refusal and a usage error share one code, or a closed pipe prints a panic.
- AC-010: The real program counts from the real clock: the end it prints is the time at which it ran plus the length, never a fixed or remembered moment.
  - evidence: cli/tests/real_program.rs "the_real_program_ends_a_session_at_the_real_clock_plus_the_length"
  - breaker: The end is a built-in moment, a time stored from an earlier run, or off by the time zone.
- AC-011: The clock is read in one place only, `cli/src/main.rs`; every other file of the command-line crate gets the current moment as an argument, as the core does.
  - evidence: cli/tests/clock_at_the_edge.rs "only_main_reads_the_clock"
  - breaker: A new file in the command-line crate reads the clock or the time zone, so a test can no longer fix the moment.

## Checklist coverage
- CLI-1 -> criterion:AC-003
- CLI-2 -> criterion:AC-009
- CLI-3 -> criterion:AC-008

## Derived checklist
Not used: Domains is cli, which has its own checklist above.

## Edge cases
- `leko session` with no length, `leko session ""`, two lengths, or an argument that is not valid text (bytes that are not UTF-8) -> AC-003
- Length 0, -5, 10081, 8d, a 30-digit number, `99999999999999999h`, full-width or Arabic-Indic digits -> AC-002
- A rule name of 65 characters, with a slash or a space, or empty (`--id ""`) -> AC-004
- `--json` before the length and after it, and given twice -> AC-005
- "Start a 25-minute session!" in capitals with punctuation, and "a half hour session" -> AC-006
- "Cancel my 30 minute session", "don't start a 10 minute session", "an hour and a half", "20 or 30 minutes" -> AC-007
- A phrase of 1 MB, or empty, or only spaces -> AC-007
- The same command run twice in a row, and run in a folder with no write permission -> AC-008
- stdout closed before the program writes (`| head -c 0`) -> AC-009
- The clock set before 1970: main.rs turns the failed clock read into exit 1 with a sentence -> out-of-scope: a test cannot set the real clock; main.rs holds only this one branch and it is read in review, not tested

## Out of scope
- Writing the session into a block, a settings zip, or telling an agent to enforce it (the decision-loop and add-a-block steps, Phases 2 and 3, with their confirm step).
- The microphone, speech recognition and the local language model (Phase 4; the runtime and licence are still open).
- Saying which block or app the session covers ("block Instagram for 30 minutes").
- Cancelling, shortening or extending a session (that is loosening or tightening and goes through the judge).
- Showing the end in the user's own time zone (needs the operating system's zone), and languages other than English.
- Packaging, installing and shell completion.

## Decisions
- A new crate `cli/` (package `leko-cli`, program `leko`) beside `core/` | the core stays free of operating system calls and of the clock | REVERSIBLE
- All logic in a library function taking the arguments, the current moment and the two output streams; `main.rs` reads the clock and passes it in | tests fix the moment, and there is no `--now` flag a user could misuse | REVERSIBLE
- Exit codes 0 made, 1 understood but refused, 2 not understood | a script and the voice layer can tell the cases apart | REVERSIBLE
- The default output is the sentence and `--json` gives one line | the voice layer reads the sentence back, scripts read the JSON | REVERSIBLE
- The default rule name is `session` | a block holds at most one session rule | REVERSIBLE
- The voice grammar is small, fixed and English, and refuses when in doubt | a wrong guess seals the user in for up to 7 days | REVERSIBLE
- No argument-parsing crate; the three flags are read by hand | exact control of the sentences and exit codes, no new dependency | REVERSIBLE
- Print only, nothing applied | nothing here can enforce a session yet, so printing is the safe first step | REVERSIBLE
