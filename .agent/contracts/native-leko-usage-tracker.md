# Contract: native-leko-usage-tracker

Written before any code. After the user opens it, the contract is locked: criteria cannot be edited to match the code.
Every "<...>" part must be replaced with real text. `/contract open native-leko-usage-tracker` refuses a contract that is not complete.

## Goal
`leko enforce` gains a live, injectable OS foreground usage and session tracker persisted beside `rules.json` so `Time Limit` and `Session Limit` enforce real usage budgets without mutating rules. This also promotes `Engagement Pass` from `[PLANNED]` to `[LIVE]` across core, CLI, and voice routing, surfaces remaining usage budgets in status output, and shows active `Time Limit` countdowns in `leko-pill` when `quietTimer` is enabled.

## Domains
Domains: cli, data, ui

## Acceptance criteria
- AC-001: Foreground app and active browser site observations flow through an injectable observer trait so all unit and CLI tests run 100% headless and deterministically without reading real user windows or live browser tabs.
  - evidence: cli/tests/status_evaluate.rs "foreground_observer_is_injected_and_deterministic"
  - breaker: A test reads real macOS windows or fails when CI has no GUI session or running browser.
- AC-002: On macOS, the system observer reads the frontmost application bundle identifier and active browser domain from existing read-only heartbeat or evaluation state without editing any protected daemon or extension path.
  - evidence: cli/src/services/os_executor.rs "system_observer_parses_frontmost_and_browser_site"
  - breaker: Active website tracking edits a protected path or misses domains present in browser heartbeats.
- AC-003: Tracked usage seconds, debounced session counts, period keys, and engagement focus seconds are saved in an atomic crash-safe sibling `.usage-state.json` (`rules.usage-state.json`) file beside `rules.json`, never mutating `TechnologyManagementRule` or its `updated_at` timestamp.
  - evidence: core/tests/native_store.rs "a_partial_usage_state_write_keeps_the_old_state"
  - breaker: A partial write corrupts tracked budgets, or tracking rewrites `rules.json`.
- AC-004: Schedule window and calendar period rollovers reset usage budgets deterministically using injected clocks, and a short Cmd-Tab focus-flicker debounce keeps rapid switching within one session instead of burning multiple sessions.
  - evidence: core/tests/native_schedule.rs "usage_budgets_roll_over_and_debounce_flickers"
  - breaker: A 2-second focus flicker burns two sessions, or yesterday's exhausted budget still blocks today.
- AC-005: `leko enforce` feeds persisted `Time Limit` and `Session Limit` state into rule evaluation every tick, keeping targets allowed inside budget and enforcing (`ForceClosePenalty`, browser warn/close, and pill `Blocked` flash) once exhausted unless a Pass is active.
  - evidence: cli/tests/status_evaluate.rs "time_and_session_limits_enforce_when_exhausted"
  - breaker: Targets are blocked before their budget expires, or remain open after exhaustion with no active Pass.
- AC-006: `leko check-status` and `leko evaluate-rule` read persisted usage state (while preserving explicit CLI `--used-minutes` and `--sessions` overrides) and report remaining `Time Limit` and `Session Limit` budgets in both `--json` and exact Title Case human output.
  - evidence: cli/tests/output_modes.rs "status_and_evaluate_report_remaining_usage_budgets"
  - breaker: Status output omits remaining usage budgets or breaks existing `--json` keys.
- AC-007: `Engagement Pass` (`PassRequirement::EngagementPass`) is promoted from `[PLANNED]` to `[LIVE]` in `core/src/`, `docs/vocab/USER_VOCABULARY.md`, `cli/` (`--engagement-pass` and `--pass` JSON), and `core/tests/native_vocabulary.rs`.
  - evidence: core/tests/native_passes.rs "engagement_pass_grants_after_required_focus_duration"
  - breaker: `EngagementPass` remains rejected by vocabulary checks or cannot be created and parsed as a rule pass.
- AC-008: Active focus seconds inside an `Engagement Pass` target accumulate across ticks (or via explicit CLI `--elapsed-seconds` on `request-pass`), granting `TimedPass` or `RestOfDayPass` in `rules.pass-state.json` once satisfied, subject to `PassCap` and without editing the rule.
  - evidence: cli/tests/pass_request.rs "engagement_pass_records_after_focus_requirement"
  - breaker: An `Engagement Pass` grants before focus duration is met, bypasses `LimitedPasses`, or mutates `rules.json`.
- AC-009: Voice routing removes `Engagement Pass` (`engagement_pass`) from Maeve's planned refusal lists and maps it with exact Title Case `"Engagement Pass"`, while `Pomodoro Schedule`, `Community-Linked Apps`, `Community-Linked Websites`, and `Restart Seal` still refuse with zero CLI or LLM calls.
  - evidence: tests/python/routing/test_voice_planned_features.py "engagement_pass_is_live_while_other_planned_features_refuse"
  - breaker: Maeve still refuses `Engagement Pass`, or stops refusing any remaining `[PLANNED]` feature.
- AC-010: When `quietTimer: true` is configured in `pill-config.json`, `leko-pill` polls the active usage countdown alongside `rules.pass-state.json`, shows monochrome `MM:SS` while inside an active `Time Limit` target, and clears with `timer_stop` back to idle when inactive (still defaulting to `quietTimer: false`).
  - evidence: tests/ui/test-pill-states.mjs "quiet_timer_tracks_active_time_limit_and_returns_idle"
  - breaker: `quietTimer: false` stops showing the idle capsule, or a stale countdown stays stuck on the pill after leaving the target.
- AC-011: Wrong or missing tracker, engagement-pass, or status arguments print usage to stderr and exit non-zero; valid output goes to stdout and errors go to stderr.
  - evidence: cli/tests/arguments.rs "usage_tracker_and_engagement_flags_validate_streams"
  - breaker: An invalid flag exits zero or writes diagnostic text into `--json` stdout.
- AC-012: Corrupt or missing `rules.usage-state.json` fails safely without crashing `leko enforce`, and repeated `--once` ticks with an empty store or unchanged focus are safe and idempotent.
  - evidence: cli/tests/store_atomic.rs "corrupt_usage_state_and_empty_ticks_are_safe"
  - breaker: A corrupt usage file crashes a `KeepAlive` agent or resets another valid store.
- AC-013: Every new or touched file stays strictly under 350 lines (`voice/session/runtime.py` untouched at <= 349 lines), every touched directory stays at or below 8 immediate entries, zero em-dashes exist anywhere, and protected paths stay byte-identical to `main`.
  - evidence: tests/python/cli/test_enforcement_loop.py "m9_shape_limits_and_protected_paths_hold"
  - breaker: Any file hits 350 lines, a directory exceeds 8 entries, an em-dash is written, or a protected file changes.
- AC-014: No test or command touches `/Users/calibro/Projects/leko-enforcer`, `~/Library/LaunchAgents/com.leko.enforcer.plist`, or any live service on 127.0.0.1:8765, and `shadow_mode` and `nsfw` remain false and non-overridable.
  - evidence: tests/python/cli/test_enforcement_loop.py "m9_no_live_targets_are_touched"
  - breaker: Any code path writes to live user paths or relaxes fixed safety flags.
- AC-015: The full verification command recorded in Decisions passes when invoked externally via `/contract done --run`, with no discovered test invoking it recursively.
  - evidence: tests/python/cli/test_enforcement_loop.py "m9_release_binary_and_tracker_smoke_verified"
  - breaker: Any binary fails to build or a test suite recurses into the full verification script.

## Checklist coverage
- CLI-1 -> criterion:AC-011
- CLI-2 -> criterion:AC-011
- CLI-3 -> criterion:AC-012
- DATA-1 -> criterion:AC-003
- DATA-2 -> criterion:AC-012
- DATA-3 -> criterion:AC-008
- UI-1 -> criterion:AC-010
- UI-2 -> criterion:AC-010
- UI-3 -> criterion:AC-010

## Derived checklist
(not used: Domains is not "none")

## Edge cases
- Rapid Cmd-Tab switching away and back within the session debounce window -> AC-004
- Calendar day or `RoutineSchedule` window rollover while an app stays open -> AC-004
- `SessionLimit` at exact `max_sessions` while the active session is still open vs re-entering after debounce -> AC-005
- Exhausting a `TimeLimit` mid-session with vs without an active `Pass` -> AC-005
- Missing, empty, or half-written `rules.usage-state.json` on startup or reload -> AC-003, AC-012
- `EngagementPass` focus interrupted before `required_seconds` vs reaching `required_seconds` at `LimitedPasses` cap -> AC-008
- Leaving a `Time Limit` target while `leko-pill` has `quietTimer: true` -> AC-010

## Out of scope
- Any change to `/Users/calibro/Projects/leko-enforcer`, `~/Library/LaunchAgents/com.leko.enforcer.plist`, or live daemon `127.0.0.1:8765`.
- Any edit to `src/daemon/`, `src/blocker-extension/`, `src/page-regions/regions.mjs`, `enforcers/`, `voice/audio_io/`, or `voice/normalization/`.
- Any growth to `voice/session/runtime.py` beyond its current 349 lines.
- Promoting `PomodoroSchedule`, `CommunityLinkedApps`, `CommunityLinkedWebsites`, or `RestartSeal` from `[PLANNED]`.
- Windows, Android, or multi-device cloud usage sync.

## Decisions
- Use a separate versioned sibling `.usage-state.json` (`rules.usage-state.json`) file beside `rules.json` instead of mutating `rules.json` | keeps `TechnologyManagementRule` and `updated_at` immutable while remaining crash-safe via atomic write+rename | REVERSIBLE
- Track second-level `active_seconds` in `usage-state.json` and evaluate `TimeLimit` at `active_seconds < minutes * 60` | prevents 59-second bypasses and powers live `MM:SS` countdowns in `leko-pill` | REVERSIBLE
- Inject an OS foreground observer trait returning frontmost app bundle and optional active site, backed on macOS by `NSWorkspace` frontmostApplication plus existing read-only heartbeat/eval state | keeps all tests headless and deterministic without editing protected daemon or extension paths | REVERSIBLE
- Default Cmd-Tab session debounce to 5 seconds (`--session-debounce-seconds` override for tests) and allow sessions `1..=max_sessions` until a post-debounce re-entry exceeds `max_sessions` | avoids punishing brief app switches and matches human expectations of a session count | REVERSIBLE
- Pass active `Time Limit` remaining seconds to `leko-pill` via atomic timer metadata in `pill-events.json` (plus `ui/src/timer_poll.rs` emitting `timer_start` and `timer_stop`) | keeps `ui/src/main.rs` well under 350 lines and respects `quietTimer: false` default | REVERSIBLE
- Model `PassRequirement::EngagementPass { targets, required_seconds }` and track progress in `usage-state.json` across `enforce` ticks (with `--elapsed-seconds` on `request-pass`) | enables automated tick unlocking without altering the rule definition | REVERSIBLE
- Place new Rust/Python/UI checks into existing 8-entry capped test directories plus allowed service modules (`core/src/services/usage_state.rs`, `cli/src/services/usage_tracker.rs`, `ui/src/timer_poll.rs`) | strictly obeys the 8-entry directory cap and `< 350` line cap | REVERSIBLE
- Verify externally via `/contract done --run` with `~/.cargo/bin/cargo build --release && ~/.cargo/bin/cargo test && /Users/calibro/.t3/scratch/phone-pilot/talk-venv/bin/python3 -m unittest discover -s tests -p 'test_*.py' -v && node --test tests/ui/*.mjs` | builds fresh release binaries and verifies Rust, Python, and UI suites with zero recursive test runs | REVERSIBLE
