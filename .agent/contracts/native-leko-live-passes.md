# Contract: native-leko-live-passes

Written before any code. After the user opens it, the contract is locked: criteria cannot be edited to match the code.
Every "<...>" part must be replaced with real text. `/contract open <feature>` refuses a contract that is not complete.

## Goal
A `Pass` earned through `leko request-pass` becomes real live state that `leko enforce` reads on every tick, so an active pass suspends enforcement until it expires without ever editing the stored rule. This also adds a real countdown wait and makes `leko delete-rule` imply a delete edit.

## Domains
Domains: cli, data

## Acceptance criteria
- AC-001: Active passes and per-window usage counts are saved in an atomic crash-safe state file, and an interrupted write leaves the previous state readable.
  - evidence: core/tests/native_store.rs "a_partial_pass_state_write_keeps_the_old_state"
  - breaker: A half-written pass state file becomes the live state, or a crash loses a previously earned pass.
- AC-002: `leko request-pass` persists the earned pass after the requirement is satisfied, honours `UnlimitedPasses` and `LimitedPasses`, and refuses with a plain error on `NoPass` or when the cap is reached.
  - evidence: cli/tests/pass_request.rs "requesting_a_pass_persists_without_editing_the_rule"
  - breaker: A pass exists only for the duration of the command, or a capped pass is granted after the cap.
- AC-003: Earning a pass never changes the stored `TechnologyManagementRule` definition or its `updated_at` value.
  - evidence: cli/tests/pass_request.rs "earning_a_pass_never_changes_the_rule_or_its_timestamp"
  - breaker: A pass writes into `rule.pass` or bumps the rule timestamp.
- AC-004: The enforce loop reads persisted pass state on every tick, so a pass earned by another CLI call immediately suspends enforcement in the running agent.
  - evidence: cli/tests/status_evaluate.rs "the_live_loop_reads_persisted_pass_state"
  - breaker: A newly earned pass is ignored until the agent restarts, or explicit `--earned-at` stops working for tests.
- AC-005: A `TimedPass` or `RestOfDayPass` resumes enforcement automatically once expired, without another request.
  - evidence: cli/tests/status_evaluate.rs "an_expired_pass_resumes_enforcement_on_the_next_tick"
  - breaker: An expired pass keeps bypassing enforcement.
- AC-006: `CountdownPass` carries a configurable `wait_seconds`, and the wait is enforced through an injectable sleeper so tests are instant and deterministic.
  - evidence: core/tests/native_passes.rs "countdown_wait_is_enforced_with_an_injected_sleeper"
  - breaker: A countdown pass grants immediately, or a test sleeps for real seconds.
- AC-007: A live `leko request-pass` waits out the configured countdown before recording the pass and reports the wait in both output modes.
  - evidence: cli/tests/pass_request.rs "a_live_countdown_waits_before_granting"
  - breaker: The CLI returns before the countdown ends, or the wait is skipped in production.
- AC-008: `check-status` and `evaluate-rule` read the same persisted pass state and report it accurately in human text and stable `--json` output.
  - evidence: cli/tests/status_evaluate.rs "status_and_json_reflect_persisted_passes"
  - breaker: Maeve says a rule is enforced while a live pass is active, or a JSON key changes.
- AC-009: Voice status replies describe a persisted active pass with canonical Title Case and do not claim the rule changed.
  - evidence: tests/python/routing/test_voice_canonical_speech.py "spoken_status_uses_persisted_pass_state"
  - breaker: Spoken status ignores an active pass or says the rule was edited.
- AC-010: `leko delete-rule` implies a delete edit by default while still accepting an explicit `--direction delete`.
  - evidence: cli/tests/rule_edit.rs "delete_rule_implies_delete_direction"
  - breaker: The old error appears when `--direction` is omitted, or an explicit delete is rejected.
- AC-011: A sealed rule still blocks, queues or requires satisfaction for deletion; the ergonomics fix does not weaken Seals.
  - evidence: cli/tests/rule_edit.rs "a_sealed_delete_still_requires_the_seal"
  - breaker: A `CountdownSeal` delete succeeds without the seal being satisfied.
- AC-012: Wrong or missing pass and delete arguments print usage to stderr and exit non-zero; results go to stdout and errors to stderr.
  - evidence: cli/tests/arguments.rs "pass_and_delete_usage_and_streams"
  - breaker: A bad invocation exits zero, or an error is printed into the JSON stream.
- AC-013: Repeated and empty pass requests are safe and idempotent, with no duplicate pass records or crash.
  - evidence: cli/tests/pass_request.rs "repeated_and_empty_pass_requests_are_safe"
  - breaker: A second request duplicates the pass or panics on an empty store.
- AC-014: Every touched file is under 350 lines, every touched directory has at most 8 immediate entries, no file contains an em-dash, protected paths stay byte-identical to `main`, and no test touches the live LaunchAgent, the old checkout or the daemon.
  - evidence: tests/python/cli/test_native_passes.py "guardrails_and_protected_paths_hold"
  - breaker: A tenth file lands in a test folder, a protected file changes, or a test writes to a live path.
- AC-015: The release binary is rebuilt and remains runnable, and both suites pass when invoked externally; the full command is recorded in Decisions for `/contract done --run`.
  - evidence: tests/python/cli/test_native_passes.py "release_binary_and_suites_are_verified_externally"
  - breaker: The release build breaks or a discovered test recurses into the full suite.

## Checklist coverage
- CLI-1 -> criterion:AC-012
- CLI-2 -> criterion:AC-012
- CLI-3 -> criterion:AC-013
- DATA-1 -> criterion:AC-001
- DATA-2 -> criterion:AC-001
- DATA-3 -> criterion:AC-002

## Derived checklist
(not used: Domains is not "none")

## Edge cases
- A pass state file that is empty or corrupt -> AC-001
- A `LimitedPasses` request at the cap -> AC-002
- A countdown that is interrupted mid-wait, so no pass is recorded -> AC-006
- A pass expiring between two enforcement ticks -> AC-005
- A `--earned-at` value from a previous period -> AC-004
- A sealed `delete-rule` with no direction flag -> AC-011
- A rule whose name contains non-ASCII characters in status output -> AC-008

## Out of scope
- Any change to `/Users/calibro/Projects/leko-enforcer`, the live LaunchAgent or the daemon on 127.0.0.1:8765.
- Any edit to `src/daemon/`, `src/blocker-extension/`, `src/page-regions/regions.mjs`, `enforcers/`, `voice/audio_io/` or `voice/normalization/`.
- New `[PLANNED]` built-ins, add-on runner changes and cross-device pass sync.
- Windows and Android enforcement, and native UI changes.

## Decisions
- Keep persisted passes in a separate versioned JSON state file beside the rule store | the rule definition and its timestamp must stay immutable, and `core/src/services` has room for one more module | REVERSIBLE
- Use an injectable sleeper for countdown waits, with real waiting only in production CLI runs | tests must stay instant and deterministic while live runs really wait | REVERSIBLE
- Key usage counts by local calendar day in the configured zone | it matches `LimitedPasses` semantics and the existing `Period::Daily` behaviour | REVERSIBLE
- Make `delete-rule` imply a delete edit while leaving Seals fully enforced | fixes the live smoke-test error without weakening rule protection | REVERSIBLE
- Put M6 tests in existing `core/tests/` and `cli/tests/` files plus one new Python test | those Rust test folders already have 8 entries and cannot grow | REVERSIBLE
- Verify with `~/.cargo/bin/cargo build --release && ~/.cargo/bin/cargo test && /Users/calibro/.t3/scratch/phone-pilot/talk-venv/bin/python3 -m unittest discover -s tests -p 'test_*.py' -v`, passed to `/contract done --run`; AC-015 keeps only external binary checks | the release build must be fresh and the full command must not run inside a discovered test | REVERSIBLE
