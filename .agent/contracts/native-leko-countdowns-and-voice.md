# Contract: native-leko-countdowns-and-voice

Written before any code. After the user opens it, the contract is locked: criteria cannot be edited to match the code.

## Goal
Users get a unified deterministic `leko countdowns` view across all live countdown types, a smart `leko-pill` that surfaces the soonest countdown within 15 minutes and expands into a monochrome reply card with optional tables when Maeve answers, and a 100% deterministic voice pipeline with the local LLM disconnected by default.

## Domains
Domains: cli, data, ui

## Acceptance criteria
- AC-001: `leko countdowns` (and `leko check-status`) computes all active and upcoming countdowns deterministically from `rules.json`, `rules.usage-state.json`, and `rules.pass-state.json` at an injected `--at` timestamp without mutating `rules.json`, covering active `Time Limit`, active `Timed Pass`, active `Engagement Pass`, upcoming `Routine Schedule` or `One Time Schedule` transitions, and pending `Queued Delay Seal` maturity.
  - evidence: core/tests/native_schedule.rs "unified_countdowns_cover_all_five_types_at_injected_clock"
  - breaker: A countdown type is omitted, computed against wall-clock time instead of `--at`, or mutates `rules.json`.
- AC-002: Unified countdowns sort deterministically by soonest `remaining_seconds` ascending, tie-broken by urgency (`Time Limit` > `Timed Pass` > `Engagement Pass` > `Schedule` > `Queued Delay Seal`), and expose `--json` plus exact Title Case human output.
  - evidence: cli/tests/output_modes.rs "countdowns_command_sorts_by_remaining_and_urgency_in_json_and_title_case"
  - breaker: Equal remaining seconds sort in arbitrary order, or human output mixes lowercase or non-canonical labels.
- AC-003: Wrong or missing `countdowns` flags print usage to stderr and exit non-zero, valid JSON or human output goes only to stdout, and running `countdowns` on an empty or corrupt usage/pass file is safe and idempotent.
  - evidence: cli/tests/arguments.rs "countdowns_flags_validate_streams_and_empty_stores"
  - breaker: Invalid `--at` or missing `--store` exits zero, or prose leaks into `--json` stdout.
- AC-004: Reading `rules.usage-state.json` and `rules.pass-state.json` for `countdowns` preserves atomic read-only semantics on missing, empty, or versioned state files without overwriting existing data.
  - evidence: cli/tests/store_atomic.rs "countdowns_handles_missing_and_corrupt_state_files_safely"
  - breaker: A missing or corrupt sibling state file crashes `leko countdowns` or overwrites state on disk.
- AC-005: `pill-config.json` supports `timerThresholdMinutes` (default `15`) alongside `quietTimer`: `leko enforce` and `leko-pill` display the single soonest monochrome `MM:SS` only when an active or upcoming countdown is `<= timerThresholdMinutes` minutes, and collapse back to the idle dash when none qualify.
  - evidence: tests/ui/test-pill-states.mjs "quiet_timer_respects_15_minute_threshold_and_returns_idle"
  - breaker: Countdowns over 15 minutes hijack the idle dash, or a finished timer stays stuck on the pill.
- AC-006: Idle hover stays strictly inside the small capsule with zero OS window resize or hover jitter, while `Blocked` flashes always preempt idle, timer, and reply-card states on enforcement.
  - evidence: tests/ui/test-pill-input.mjs "idle_hover_never_resizes_window_and_blocked_takes_priority"
  - breaker: Hovering the idle pill resizes the native OS window or blocks an incoming `Blocked` flash.
- AC-007: When Maeve emits a structured reply payload over UI IPC, `leko-pill` expands into a compact monochrome rounded rectangle that streams spoken text, renders an optional 2-to-3 column table, pauses auto-collapse while hovered, and collapses cleanly after speaking, on outside click, or on Escape.
  - evidence: tests/ui/test-pill-states.mjs "reply_card_streams_text_renders_table_and_handles_hover_and_escape"
  - breaker: The reply card clips text, auto-collapses while hovered, ignores Escape, or uses non-monochrome accents.
- AC-008: The local LLM (LFM) is disconnected and bypassed on the default voice talking path (only opt-in via explicit flag/env), so `process_query` and default routing complete with zero HTTP or `llama-server` calls.
  - evidence: tests/python/routing/test_voice_native_commands.py "default_voice_path_never_calls_local_llm"
  - breaker: Default voice routing attempts to launch `llama-server` or send HTTP completions to port 18080.
- AC-009: Deterministic intent matching and pre-scripted templates handle countdowns, rules list, active status/blocks, rule remaining time, active passes, unambiguous create/delete/request-pass actions, and a concise pre-scripted fallback when unmatched, emitting spoken text plus structured table rows over `UIPipelineBridge`.
  - evidence: tests/python/session/test_query_flow.py "deterministic_voice_routes_queries_and_emits_reply_card"
  - breaker: Voice queries fall back to LLM generation, fabricate status text, or omit structured card rows.
- AC-010: `[PLANNED]` feature names (`Pomodoro Schedule`, `Community-Linked Apps`, `Community-Linked Websites`, `Restart Seal`) continue to return the fixed refusal with zero CLI and zero LLM calls.
  - evidence: tests/python/routing/test_voice_planned_features.py "planned_features_refuse_without_cli_or_llm"
  - breaker: A planned feature phrase hits the deterministic CLI or invokes the legacy LLM router.
- AC-011: Every new or touched code file stays strictly under 350 lines (`voice/session/runtime.py`, `voice/core/assistant.py`, and `voice/session/query_flow.py` stay `<= 349` lines), every touched directory stays at or below 8 immediate entries, zero em-dashes exist anywhere, and protected paths stay byte-identical to `main`.
  - evidence: tests/python/cli/test_enforcement_loop.py "countdowns_and_voice_shape_and_protected_paths_hold"
  - breaker: Any file reaches 350 lines, a directory exceeds 8 files, an em-dash is introduced, or a protected path changes.
- AC-012: No test touches `/Users/calibro/Projects/leko-enforcer`, live LaunchAgents, or port `8765`, `shadow_mode` and `nsfw` remain false and non-overridable, and the full verification suite passes via `/contract done --run`.
  - evidence: tests/python/cli/test_enforcement_loop.py "m9_no_live_targets_are_touched"
  - breaker: A test writes to live user paths, modifies safety flags, or fails under the release verification suite.

## Checklist coverage
- CLI-1 -> criterion:AC-003
- CLI-2 -> criterion:AC-003
- CLI-3 -> criterion:AC-003
- DATA-1 -> criterion:AC-004
- DATA-2 -> criterion:AC-004
- DATA-3 -> criterion:AC-004
- UI-1 -> criterion:AC-007
- UI-2 -> criterion:AC-007
- UI-3 -> criterion:AC-007

## Derived checklist
(not used: Domains is not "none")

## Edge cases
- Empty store, all rules disabled, or all countdowns strictly greater than `timerThresholdMinutes` (`15m`) -> AC-005
- Tied `remaining_seconds` across `Time Limit`, `Timed Pass`, `Engagement Pass`, `Schedule`, and `Queued Delay Seal` -> AC-002
- Unrecognized spoken utterance or missing rule name in deterministic voice routing (must hit pre-scripted fallback, never LFM) -> AC-009
- Hovering on the reply card just as TTS finishes, then moving cursor away or pressing Escape -> AC-007
- Missing or corrupt `rules.usage-state.json` / `rules.pass-state.json` when querying `leko countdowns` -> AC-004

## Out of scope
- Any edit to `src/daemon/`, `src/blocker-extension/`, `src/page-regions/regions.mjs`, `enforcers/`, `voice/audio_io/`, or `voice/normalization/`.
- Any growth of `voice/session/runtime.py` beyond 349 lines.
- Touching `/Users/calibro/Projects/leko-enforcer`, `~/Library/LaunchAgents/com.leko.enforcer.plist`, or port `8765` in code or tests.
- Multi-turn open-ended LLM conversation or re-enabling LFM by default.

## Decisions
- Add unified countdown computation in `core/src/countdowns.rs` and expose `leko countdowns` via `cli/src/commands/check_status.rs` | avoids overflowing the 8-file directory limits in `core/src/domain/` and `cli/src/commands/` | REVERSIBLE
- Represent countdown filter threshold as `timerThresholdMinutes` (default `15`) in `pill-config.json` and pass soonest countdown metadata through `pill-events.json` | keeps UI logic thin and aligned with `leko-core` countdown priority | REVERSIBLE
- Render the expandable monochrome reply card via `ui/src/card.rs` and `ui/src/web/card.css` inside `leko-pill`, keeping idle hover strictly inside the capsule and resizing only for the reply card | guarantees zero idle hover jitter while displaying multi-column tables cleanly | REVERSIBLE
- Disconnect LFM from default voice routing and place deterministic pattern matching and templates in `voice/routing/deterministic_router.py` (keeping `llm_router.py` opt-in behind `LEKO_ENABLE_LLM_ROUTER=1`) | delivers instant deterministic replies while keeping existing opt-in LLM tests intact | REVERSIBLE
- Verify externally via `/contract done --run` with `~/.cargo/bin/cargo build --release && ~/.cargo/bin/cargo test && /Users/calibro/.t3/scratch/phone-pilot/talk-venv/bin/python3 -m unittest discover -s tests -p 'test_*.py' -v && node --test tests/ui/*.mjs` | builds fresh release binaries and verifies Rust, Python, and UI suites with zero recursive test runs | REVERSIBLE
