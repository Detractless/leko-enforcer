# Contract: native-leko-enforcer-loop

Written before any code. After the user opens it, the contract is locked: criteria cannot be edited to match the code.
Every "<...>" part must be replaced with real text. `/contract open <feature>` refuses a contract that is not complete.

## Goal
`leko enforce STORE.json` becomes a continuous, safe OS enforcement loop that applies native `decide()` outcomes to apps, websites and whole-device rules. It replaces the one-shot decision printer while keeping tests deterministic through an injected OS executor, and preserves the protected daemon and extension surface.

## Domains
Domains: cli, data

## Acceptance criteria
- AC-001: `leko enforce STORE.json` runs a continuous loop with a roughly one-second tick by default, reloads the native store each tick, and stops cleanly on a shutdown signal or `--once`.
  - evidence: cli/tests/status_evaluate.rs "the_loop_reloads_the_store_on_every_tick"
  - breaker: The command prints one decision set and exits, or a changed store is never picked up.
- AC-002: For `SpecificApps` and `AllowOnlyApps` with `ForceClosePenalty`, the loop enumerates running apps and terminates enforced apps on every reopen, skipping its own process and protected system apps, and does nothing while a Pass is active or the Schedule is inactive.
  - evidence: cli/tests/status_evaluate.rs "blocked_apps_are_terminated_until_the_pass_or_schedule_stops_them"
  - breaker: A permitted app is killed, or a blocked app is not retried after reopening.
- AC-003: For `SpecificWebsites`, `TopicDetectedWebsites` and `AllowOnlyWebsites`, the loop preserves browser heartbeat judgement and warn/close actions through the protected native enforcement contract and native messaging coordination.
  - evidence: cli/tests/status_evaluate.rs "browser_heartbeats_drive_warn_and_close"
  - breaker: A detached extension is treated as healthy, or a locked block never warns or closes a browser.
- AC-004: For `WholeDevice`, the loop triggers `Lock`, `Logoff`, `Restart` or `Shutdown` continuously while enforced and respects active Passes; an unavailable action is reported plainly rather than panicking.
  - evidence: cli/tests/status_evaluate.rs "whole_device_actions_follow_the_native_device_action"
  - breaker: A device action is skipped during enforcement, or a Restart attempt aborts the loop.
- AC-005: All OS interactions go through an injected executor trait so tests can substitute mocks that never kill a process, lock a screen or call the live daemon.
  - evidence: cli/src/services/os_executor.rs "mock_executors_never_touch_the_real_machine"
  - breaker: A test path reaches a real macOS API or the daemon on 127.0.0.1:8765.
- AC-006: `--once` performs one pass and exits, `--dry-run` reports intended actions without calling the executor, and `--interval-ms` controls the tick interval.
  - evidence: cli/tests/arguments.rs "once_and_dry_run_flags_keep_tests_safe"
  - breaker: A dry run still terminates an app, or a chosen interval is ignored.
- AC-007: The loop is compatible with `KeepAlive: true` launch agents, shuts down on clean signals and never crashes on a transient store problem.
  - evidence: cli/tests/guardrails.rs "keepalive_compatible_loop_shuts_down_cleanly"
  - breaker: A signal leaves a half-written action, or a transient error exits the process and launchd restarts it in a tight loop.
- AC-008: A missing, empty or corrupt store is reported plainly, the last valid state is kept and failures never fail open.
  - evidence: cli/tests/store_atomic.rs "a_corrupt_or_missing_store_keeps_the_last_valid_state"
  - breaker: A corrupt store disables enforcement, or the file is overwritten with defaults.
- AC-009: The native store version and keys are preserved, with no `.ctsettings` import or legacy field introduced.
  - evidence: cli/tests/store_atomic.rs "native_store_version_is_preserved"
  - breaker: The loop rewrites the store into a legacy shape or bumps its version silently.
- AC-010: Wrong or missing arguments print a usage line to stderr and exit non-zero; results go to stdout, errors to stderr.
  - evidence: cli/tests/arguments.rs "usage_and_streams_follow_the_cli_contract"
  - breaker: A bad invocation exits zero, or an error sentence is printed into a JSON stream.
- AC-011: Running the command twice on empty or unchanged input is safe and says what it did.
  - evidence: cli/tests/arguments.rs "repeated_and_empty_enforce_requests_are_safe"
  - breaker: A second loop duplicates actions or panics on an empty store.
- AC-012: Pure judgement takes the injected clock and timestamp, and no clock read appears outside the CLI clock adapter.
  - evidence: cli/tests/status_evaluate.rs "injected_clock_keeps_the_loop_deterministic"
  - breaker: A hidden system clock read makes two identical runs disagree.
- AC-013: Every touched file is under 350 lines, every touched directory has at most 8 immediate entries, no file contains an em-dash and protected paths stay byte-identical to `main`.
  - evidence: tests/python/cli/test_enforcement_loop.py "shape_limits_and_protected_paths_hold"
  - breaker: A new ninth file lands in `cli/tests/`, a protected file changes, or an em-dash is added.
- AC-014: No test or command touches `/Users/calibro/Projects/leko-enforcer`, `~/Library/LaunchAgents/com.leko.enforcer.plist` or any live service.
  - evidence: tests/python/cli/test_enforcement_loop.py "no_live_targets_are_touched"
  - breaker: A test writes to the old checkout or changes a loaded launch service.
- AC-015: The release binary is rebuilt from this branch and remains runnable, and both suites pass when invoked externally; the full command is recorded in Decisions for `/contract done --run`.
  - evidence: tests/python/cli/test_enforcement_loop.py "release_binary_and_suites_are_verified_externally"
  - breaker: The release build breaks, or a discovered test tries to run the whole suite recursively.

## Checklist coverage
- CLI-1 -> criterion:AC-010
- CLI-2 -> criterion:AC-010
- CLI-3 -> criterion:AC-011
- DATA-1 -> criterion:AC-008
- DATA-2 -> criterion:AC-008
- DATA-3 -> criterion:AC-009

## Derived checklist
(not used: Domains is not "none")

## Edge cases
- An empty native store at first tick -> AC-011
- A `--dry-run` request with a blocked app -> AC-006
- The store is deleted between two ticks, so reload fails halfway -> AC-008
- An app reopens immediately after being terminated -> AC-002
- A browser heartbeat goes stale while a block is locked -> AC-003
- A `WholeDevice` restart action that the adapter cannot perform -> AC-004
- A corrupt store file whose bytes must remain unchanged -> AC-008

## Out of scope
- Any change to `/Users/calibro/Projects/leko-enforcer`, the live LaunchAgent or the daemon on 127.0.0.1:8765.
- Any edit to `src/daemon/`, `src/blocker-extension/`, `src/page-regions/regions.mjs`, `enforcers/`, `voice/audio_io/` or `voice/normalization/`.
- Legacy `.ctsettings` or `BlockSpec` import, and new `[PLANNED]` built-ins.
- Windows and Android enforcement, cross-device sync and native UI work.

## Decisions
- Keep continuous loop as the default and make `--once` the opt-out | the live LaunchAgent uses `KeepAlive: true` and the old binary was a loop, so one-shot default would stop enforcement | REVERSIBLE
- Keep `KeepAlive: true` in generated agent plists | the loop now has clean signal shutdown and is meant to run continuously | REVERSIBLE
- Inject an OS executor trait rather than calling macOS APIs directly from judgement code | tests must never kill apps or lock the screen, and platform code already supplies narrow adapters | REVERSIBLE
- Read browser heartbeats through the protected native enforcement contract and native messaging coordination without editing protected paths | website enforcement already depends on the extension and daemon | REVERSIBLE
- Report `Restart` as unavailable when the macOS adapter cannot perform it | the current adapter refuses Restart and inventing an Apple event would be untested OS behavior | REVERSIBLE
- Verify with `~/.cargo/bin/cargo build --release && ~/.cargo/bin/cargo test && /Users/calibro/.t3/scratch/phone-pilot/talk-venv/bin/python3 -m unittest discover -s tests -p 'test_*.py' -v`, passed to `/contract done --run`; AC-015 keeps only external binary checks | the release build must run first so `target/release/leko` is not stale from M4, and the full command must not run from inside a discovered test | REVERSIBLE
