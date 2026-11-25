# Contract: native-leko-voice-routing

Written before any code. After the user opens it, the contract is locked: criteria cannot be edited to match the code.
Every "<...>" part must be replaced with real text. `/contract open <feature>` refuses a contract that is not complete.

## Goal
Maeve's voice routing calls the native `leko` CLI from Milestone 2 and speaks the exact canonical 5-pillar vocabulary from `docs/vocab/USER_VOCABULARY.md`. Legacy tool names, legacy flags, the `blocked_hours` inversion hack and legacy Door and Lock wording are removed, while `[PLANNED]` features get a fixed spoken refusal with no CLI call.

## Domains
Domains: cli, ui

## Acceptance criteria
- AC-001: Every voice tool and CLI argument list uses the ten canonical commands `create-rule`, `list-rules`, `show-rule`, `edit-rule`, `delete-rule`, `queue-edit`, `cancel-edit`, `request-pass`, `check-status` and `evaluate-rule` with kebab-case flags, and no legacy tool name or flag remains in `voice/routing/`.
  - evidence: tests/python/routing/test_voice_native_commands.py "voice_tools_map_only_to_canonical_cli_commands"
  - breaker: A spoken request still builds `add-block`, `session`, `switch-off`, `status`, `--open-hours`, `--wall` or `--write`.
- AC-002: `blocked_hours` inversion and legacy Door, Lock and Block vocabulary are gone from routing and spoken replies, replaced by `Target`, `Schedule`, `Penalty`, `Pass` and `Seal` wording.
  - evidence: tests/python/routing/test_voice_native_commands.py "no_legacy_hours_or_legacy_vocabulary_remain"
  - breaker: A validator still computes open hours from blocked hours, or a reply says Door or Lock.
- AC-003: Routing parses the stable `--json` output of `list-rules`, `show-rule`, `check-status` and `evaluate-rule` by pillar keys `target`, `schedule`, `penalty`, `pass` and `seal`, and no longer parses plain-text CLI stdout.
  - evidence: tests/python/cli/test_native_cli_json.py "voice_parses_stable_pillar_json_not_plain_text"
  - breaker: A change to human stdout wording breaks a spoken status, or a pillar key is read from the wrong place.
- AC-004: Confirmations, status readouts and errors speak exact Title Case canonical names such as `Technology Management Rule`, `Force Close Penalty`, `Queued Delay Seal`, `Whole Device`, `Routine Schedule`, `Timed Pass` and `No Seal`.
  - evidence: tests/python/routing/test_voice_canonical_speech.py "spoken_replies_use_exact_title_case_vocabulary"
  - breaker: A reply says "technology management rule" in lower case, or uses a legacy word for a pillar.
- AC-005: The spoken names `Pomodoro Schedule`, `Community-Linked Apps`, `Community-Linked Websites`, `Engagement Pass` and `Restart Seal` get the fixed "not available yet" reply and cause zero CLI calls.
  - evidence: tests/python/routing/test_voice_planned_features.py "planned_features_reply_without_any_cli_call"
  - breaker: Routing invents a CLI flag for a planned feature, or silently says nothing.
- AC-006: A seal-refused weakening is spoken as blocked by the named seal, and no reply claims the rule was edited.
  - evidence: tests/python/routing/test_voice_canonical_speech.py "a_seal_refusal_is_spoken_as_blocked_not_edited"
  - breaker: Maeve says a rule was changed after the CLI returned a seal refusal.
- AC-007: A queued delay edit is spoken as queued with its delay, and a cancelled edit is spoken as cancelled before it applies.
  - evidence: tests/python/routing/test_voice_canonical_speech.py "queued_and_cancelled_edits_are_spoken_accurately"
  - breaker: A queued edit is reported as applied, or a cancellation is reported after the change landed.
- AC-008: A pass request is spoken as a pass earned, and the reply never claims the stored rule changed.
  - evidence: tests/python/routing/test_voice_canonical_speech.py "an_earned_pass_is_not_reported_as_a_rule_edit"
  - breaker: Earning a pass produces a spoken confirmation that the rule was edited.
- AC-009: Routing and verbalization tests invoke the built `leko` binary with a temporary `--store` path and an injected `--at` timestamp, and never touch the user's real store or the live daemon on 127.0.0.1:8765.
  - evidence: tests/python/routing/test_voice_native_commands.py "tests_use_a_temp_store_and_an_injected_timestamp"
  - breaker: A test reads the real settings store, or two runs with the same timestamp disagree.
- AC-010: A spoken request with no rules says so plainly, a starting model says it is still loading, and a failed CLI call says what failed without inventing a result.
  - evidence: tests/python/routing/test_voice_canonical_speech.py "empty_loading_and_error_states_are_spoken"
  - breaker: An empty rule list is described as a failure, or a refused command is reported as success.
- AC-011: Every routing action is available by voice alone, with no mouse or rendered control required to confirm, cancel or correct a request.
  - evidence: tests/python/routing/test_voice_canonical_speech.py "all_actions_are_completable_by_voice_alone"
  - breaker: A confirm or cancel step requires clicking a UI element.
- AC-012: Missing or wrong spoken arguments produce a usage-style spoken error and a non-zero CLI result, with no crash and no fabricated rule.
  - evidence: tests/python/routing/test_voice_native_commands.py "missing_or_wrong_inputs_fail_cleanly"
  - breaker: A missing id creates an empty rule, or an exception escapes the routing layer.
- AC-013: Repeating a safe spoken command twice does not duplicate a rule or a pass, and the second reply says what it did.
  - evidence: tests/python/routing/test_voice_native_commands.py "repeated_spoken_commands_are_idempotent"
  - breaker: Saying the same create request twice stores two rules.
- AC-014: Every touched or new file is under 350 lines, every rewritten directory has at most 8 immediate entries, no file contains an em-dash, protected paths stay byte-identical to `main`, `shadow_mode` stays false and `nsfw` stays non-overridable.
  - evidence: tests/python/cli/test_native_cli_json.py "shape_limits_and_protected_paths_hold"
  - breaker: `tests/python/voice/` gains a ninth file, `voice/session/runtime.py` reaches 350 lines, or a protected file is edited.
- AC-015: No test under `tests/python/` asserts a legacy CLI command or flag, and the built `leko` binary is runnable from Python; the full suite check is recorded in Decisions and run with `/contract done --run`.
  - evidence: tests/python/cli/test_native_cli_json.py "python_tests_have_no_legacy_cli_and_the_built_binary_runs"
  - breaker: A Python test still calls `add-block` or `--open-hours`, or a Python subprocess call to `leko` cannot start.

## Checklist coverage
- CLI-1 -> criterion:AC-012
- CLI-2 -> criterion:AC-003
- CLI-3 -> criterion:AC-013
- UI-1 -> criterion:AC-010
- UI-2 -> criterion:AC-011
- UI-3 -> out-of-scope: this feature is spoken only and renders no window or text layout to resize.

## Derived checklist
(not used: Domains is not "none")

## Edge cases
- An empty store with no rules -> AC-010
- A spoken request for `Pomodoro Schedule` or `Engagement Pass` -> AC-005
- A CLI call that succeeds but returns malformed JSON, so the reply fails halfway -> AC-003
- A `Queued Delay Seal` edit cancelled one second before it applies -> AC-007
- A pass that expires between the CLI call and the spoken confirmation -> AC-008
- The same create request spoken twice in one session -> AC-013
- A non-ASCII rule name spoken back through the protected verbalizer -> AC-004

## Out of scope
- The out-of-process add-on runner and declarative recipe execution (milestone 4, `native-leko-addon-runner`).
- Any legacy CLI shim, `.ctsettings` import or legacy tool name kept for compatibility.
- Any edit to `voice/normalization/`, `voice/audio_io/`, `src/daemon/`, `src/blocker-extension/`, `src/page-regions/regions.mjs` or `enforcers/`.
- New macOS, Windows or Android enforcement behavior, and cross-device sync.
- Changes to the live daemon's HTTP endpoints or the native Rust engine.

## Decisions
- Keep `voice/normalization/` byte-identical and enforce canonical Title Case in the routing reply before it reaches the protected verbalizer | the verbalizer is a protected path but still used for speech, and changing it would break the byte-identical guardrail | REVERSIBLE
- Resolve the CLI binary from the existing `LEKO_BIN` constant with an environment override for tests | keeps production behavior while letting tests inject a temp binary and store | REVERSIBLE
- Recognize planned names in a small explicit set and answer with one fixed "not available yet" sentence, optionally naming a live alternative | makes the behavior testable without inventing any CLI surface | REVERSIBLE
- Put new routing tests in `tests/python/routing/` and new CLI JSON tests in `tests/python/cli/` | `tests/python/voice/` already has 8 entries and cannot grow | REVERSIBLE
- Verify the whole change with `~/.cargo/bin/cargo test && python3 -m unittest discover -s tests -p 'test_*.py' -v`, passed to `/contract done --run`; AC-015 keeps only the non-recursive unit check | the user requires both suites clean, the repository uses unittest, and the full command must not be invoked from inside a discovered test | REVERSIBLE
