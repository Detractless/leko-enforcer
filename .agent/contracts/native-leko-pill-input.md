# Contract: native-leko-pill-input

Written before any code. After the user opens it, the contract is locked: criteria cannot be edited to match the code.
Every "<...>" part must be replaced with real text. `/contract open <feature>` refuses a contract that is not complete.

## Goal
The floating pill reacts to hover and to the first click or hold even while another macOS app has focus. A native cursor poll and click-through control feed the existing WebView state machine, without ever taking keyboard focus or resizing the window.

## Domains
Domains: ui, cli

## Acceptance criteria
- AC-001: A native global cursor poller reads the mouse position outside the WebView and reports it through an injectable cursor provider, so tests can supply coordinates without reading the real mouse.
  - evidence: tests/ui/test-pill-input.mjs "cursor_poll_uses_an_injectable_provider"
  - breaker: The pill only reacts to WebKit mouse events, so hover never arrives while another app has focus.
- AC-002: The poller hit-tests the fixed pill rectangle and emits `hover_on` and `hover_off` into the WebView before any click.
  - evidence: tests/ui/test-pill-input.mjs "cursor_inside_and_outside_emit_hover_transitions"
  - breaker: Hover is detected only after a click, or leaving the pill never returns it to idle.
- AC-003: Polling cadence adapts between 16 ms near or over the pill and 90 ms away from it, and the intervals are testable without real waiting.
  - evidence: tests/ui/test-pill-input.mjs "poll_cadence_adapts_to_proximity"
  - breaker: A fixed high-frequency poll burns CPU while the cursor is far away.
- AC-004: The pill window is click-through by default, with mouse moves forwarded, and becomes mouse-accepting only while the cursor is inside its hit region.
  - evidence: tests/ui/test-pill-input.mjs "click_through_flips_only_while_inside"
  - breaker: A click over the pill falls through to the app behind it, or the pill captures clicks from elsewhere.
- AC-005: The macOS window accepts the first mouse-down without activating, so one click or one hold triggers the voice action.
  - evidence: tests/ui/test-pill-input.mjs "first_mouse_down_is_accepted_without_activation"
  - breaker: The first click only focuses the pill and a second click is required to trigger dictation.
- AC-006: The active app keeps keyboard focus before, during and after pill interaction.
  - evidence: tests/ui/test-pill-input.mjs "interaction_never_steals_keyboard_focus"
  - breaker: Clicking or holding the pill activates it and takes focus from the user's app.
- AC-007: Hold-to-talk over 200 ms emits `ptt_start` on press and `ptt_end` on release, and a quick click emits a start and end pair.
  - evidence: tests/ui/test-pill-input.mjs "hold_and_click_still_emit_the_same_actions"
  - breaker: The new hit-test layer changes the existing pointer gesture protocol.
- AC-008: Hover visual expansion is driven by the native poll, with the existing CSS transition and fixed window untouched.
  - evidence: tests/ui/test-pill-states.mjs "idle_pill_is_a_tiny_textless_capsule"
  - breaker: Hover works but resizes the OS window or breaks the idle capsule geometry.
- AC-009: The voice bridge IPC on stdin and stdout stays unchanged: state events in, `ptt_start` and `ptt_end` actions out.
  - evidence: tests/ui/test-pill-bridge.mjs "legacy_ui_and_voice_events_are_absent"
  - breaker: A new input path changes the JSON event names the Python bridge expects.
- AC-010: Cursor polling and click-through are isolated behind platform hooks, with a harmless fallback on non-macOS targets.
  - evidence: tests/ui/test-pill-input.mjs "platform_hooks_keep_non_macos_builds_safe"
  - breaker: A shared source file fails to compile on Windows or Linux.
- AC-011: Headless tests never open a window, grab the microphone or read the real cursor.
  - evidence: tests/ui/test-pill-input.mjs "headless_input_tests_are_deterministic"
  - breaker: CI opens a panel or depends on where the test runner's pointer is.
- AC-012: The blocked flash, quiet timer and dictation states keep their sizes, text and bar counts.
  - evidence: tests/ui/test-pill-states.mjs "blocked_flash_widens_then_shrinks"
  - breaker: Input work regresses an existing pill state.
- AC-013: Every touched file is under 350 lines, every touched directory has at most 8 immediate entries, no file contains an em-dash and protected paths stay byte-identical to `main`.
  - evidence: tests/python/cli/test_native_passes.py "shape_limits_and_protected_paths_hold"
  - breaker: A new test file pushes `tests/ui/` past 8 entries or a protected file changes.
- AC-014: No live LaunchAgent, daemon, settings store or pass state is touched by the fix or its tests.
  - evidence: tests/python/voice/test_push_to_talk_flow.py "dictation_trigger_does_not_edit_protected_voice_files"
  - breaker: A test writes to the live app support directory or restarts a service.
- AC-015: The full verification command is recorded in Decisions and passes when invoked externally, with no test running it recursively.
  - evidence: tests/ui/test-pill-input.mjs "release_binary_and_full_commands_are_verified_externally"
  - breaker: The release build breaks or a discovered test starts the whole suite.

## Checklist coverage
- UI-1 -> criterion:AC-008
- UI-2 -> criterion:AC-007
- UI-3 -> criterion:AC-008
- CLI-1 -> out-of-scope: the fix adds no CLI command.
- CLI-2 -> criterion:AC-009
- CLI-3 -> out-of-scope: no new repeatable CLI action is introduced.

## Derived checklist
(not used: Domains is not "none")

## Edge cases
- The cursor sits exactly on the pill border -> AC-002
- The cursor leaves the pill while a hold is active -> AC-005
- A click starts in another app and ends over the pill -> AC-007
- The provider returns no position -> AC-001
- The pill is behind a full-screen app -> AC-006
- A non-macOS build runs the same code path -> AC-010

## Out of scope
- Any change to `/Users/calibro/Projects/leko-enforcer`, the live LaunchAgent or the daemon on 127.0.0.1:8765.
- Any edit to `src/daemon/`, `src/blocker-extension/`, `src/page-regions/regions.mjs`, `enforcers/`, `voice/audio_io/` or `voice/normalization/`.
- Changing the fixed 160 by 56 WebView window, the CSS state animation, or the meaning of existing voice actions.
- Copying token-monitor source code; only the architectural pattern is borrowed.

## Decisions
- Poll the native cursor with `NSEvent.mouseLocation` through AppKit and inject the position into a provider trait | it works while another app has focus, and a trait keeps tests deterministic | REVERSIBLE
- Use adaptive 16 ms near and 90 ms away polling, modelled on token-monitor | matches proven behaviour without constant high-frequency polling | REVERSIBLE
- Toggle native `ignoresMouseEvents` on hit-test instead of leaving the pill always mouse-accepting | click-through is what lets the first hover work without stealing input from the active app | REVERSIBLE
- Drive hover visuals by injecting `hover_on` and `hover_off` into the existing WebView state machine | keeps CSS animation and the fixed window unchanged | REVERSIBLE
- Put M8 tests in one new file under `tests/ui/` and extend existing test files where names already exist | `tests/ui/` has room and the Rust test folders are at 8 entries | REVERSIBLE
- Verify with `~/.cargo/bin/cargo build --release && ~/.cargo/bin/cargo test && /Users/calibro/.t3/scratch/phone-pilot/talk-venv/bin/python3 -m unittest discover -s tests -p 'test_*.py' -v && node --test tests/ui/*.mjs`, passed to `/contract done --run` | every component must build and no test may recurse into the full command | REVERSIBLE
