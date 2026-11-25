# Contract: native-leko-voice-pill

Written before any code. After the user opens it, the contract is locked: criteria cannot be edited to match the code.
Every "<...>" part must be replaced with real text. `/contract open <feature>` refuses a contract that is not complete.

## Goal
Leko gets one ultra-minimal floating macOS voice pill that replaces the circle and wide Maeve UI. It shows enforcement flashes, dictation and an optional quiet timer in a single monochrome always-on-top panel, wired to the native CLI and voice pipeline.

## Domains
Domains: ui, cli

## Acceptance criteria
- AC-001: The circle and wide Maeve window UI is retired, leaving one floating pill as the only visual interface, and the legacy UI files are removed rather than kept as a second surface.
  - evidence: tests/ui/test_pill_states.mjs "one_unified_pill_replaces_the_circle_ui"
  - breaker: A second window or circle UI remains launchable alongside the pill.
- AC-002: The idle pill is a tiny textless monochrome capsule about 54 by 16 points.
  - evidence: tests/ui/test_pill_states.mjs "idle_pill_is_a_tiny_textless_capsule"
  - breaker: Idle state shows text, a dot or a larger window.
- AC-003: When enforcement blocks or closes an app, site or device action, the pill widens to show `Blocked` for about two seconds and then shrinks back to idle.
  - evidence: tests/ui/test_pill_states.mjs "blocked_flash_widens_then_shrinks"
  - breaker: A block produces no flash, or the pill never returns to idle.
- AC-004: Holding or clicking the pill starts Maeve dictation with minimal waveform bars, submits the utterance to the native v2 voice pipeline, shows a brief speaking state and returns to idle.
  - evidence: tests/ui/test_pill_states.mjs "dictation_states_flow_from_hold_to_idle"
  - breaker: The pill captures without a trigger, or a submitted utterance never reaches the v2 pipeline.
- AC-005: Quiet timer mode is off by default and only shows a compact MM:SS countdown while an active `TimedPass` is running.
  - evidence: tests/ui/test_pill_states.mjs "quiet_timer_only_appears_for_an_active_timed_pass"
  - breaker: A timer is shown by default or after the pass expires.
- AC-006: The panel is always-on-top and non-activating, so it never steals keyboard focus from the current app.
  - evidence: tests/ui/test_pill_states.mjs "panel_configuration_is_non_activating_and_always_on_top"
  - breaker: The pill activates on launch and takes focus from the user's work.
- AC-007: Placement is configurable across bottom-center, bottom-right, top-right and bottom-left and persists in user config.
  - evidence: tests/ui/test_pill_bridge.mjs "placement_anchor_persists_in_user_config"
  - breaker: A chosen anchor resets on restart or is ignored.
- AC-008: Both `dark` and `light` themes are strictly monochrome, with no accent colors, status dots, chevrons, expandable lists or menus, and text stays readable at the smallest and largest pill sizes.
  - evidence: tests/ui/test_pill_states.mjs "both_themes_are_monochrome_without_dots_or_chevrons"
  - breaker: An accent color or chevron appears, or text is clipped when widened.
- AC-009: The event bridge from `leko enforce` to the pill is an atomic event file beside the rule store with deterministic reads and writes, and no live service is contacted by tests.
  - evidence: tests/ui/test_pill_bridge.mjs "atomic_event_file_bridge_is_deterministic"
  - breaker: A half-written event is read as valid, or a test touches the live daemon.
- AC-010: The pill builds with the installed Swift compiler and AppKit without adding Rust UI dependencies.
  - evidence: tests/ui/test_pill_bridge.mjs "swift_pill_builds_without_new_rust_dependencies"
  - breaker: The build needs wry, tao or a new crate, or the Swift source does not compile.
- AC-011: State transitions and bridge reads and writes are tested headlessly with no real microphone, no real macOS window and no live LaunchAgent.
  - evidence: tests/ui/test_pill_states.mjs "headless_tests_never_grab_the_microphone_or_open_windows"
  - breaker: CI opens a panel or captures audio.
- AC-012: Core voice IPC actions and states (`ptt_start`, `ptt_end`, `idle`, `listening`, `held`, `speaking`, `thinking`, `error`) are kept for `runtime.py` and Python voice tests, while circle-only UI files and tests (`ui/web/`, `leko-ui`, circle and draft-card `.mjs` tests) are retired or replaced so `node --test tests/ui/*.mjs` passes cleanly.
  - evidence: tests/ui/test_pill_bridge.mjs "legacy_ui_and_voice_events_are_absent"
  - breaker: A core voice action is removed, a circle-only test is left behind, or a draft-card or chevron event is still emitted.
- AC-013: Every touched file is under 350 lines, every touched directory has at most 8 immediate entries, no file contains an em-dash and protected paths stay byte-identical to `main`.
  - evidence: tests/python/cli/test_native_passes.py "shape_limits_and_protected_paths_hold"
  - breaker: `tests/ui/` grows past 8 entries, `voice/session/runtime.py` reaches 350 lines, or a protected file changes.
- AC-014: Dictation is triggered through the existing voice entrypoints without editing `voice/audio_io/`, `voice/normalization/` or growing `voice/session/runtime.py` beyond 349 lines.
  - evidence: tests/python/voice/test_push_to_talk_flow.py "dictation_trigger_does_not_edit_protected_voice_files"
  - breaker: A protected audio file changes, or runtime.py grows by one line past its cap.
- AC-015: The full verification command is recorded in Decisions and passes when invoked externally, with no test running the command recursively.
  - evidence: tests/ui/test_pill_bridge.mjs "release_binary_and_full_commands_are_verified_externally"
  - breaker: The release build breaks, or a discovered test starts the whole suite.

## Checklist coverage
- UI-1 -> criterion:AC-004
- UI-2 -> criterion:AC-004
- UI-3 -> criterion:AC-008
- CLI-1 -> out-of-scope: the pill adds no new CLI command; it consumes the existing enforce and settings commands.
- CLI-2 -> out-of-scope: no new CLI output path is introduced.
- CLI-3 -> out-of-scope: no new repeatable CLI action is introduced.

## Derived checklist
(not used: Domains is not "none")

## Edge cases
- An enforcement event arrives while the pill is in dictation -> AC-003
- A quiet timer is enabled with no active pass -> AC-005
- The event file is truncated between two reads -> AC-009
- The user holds the pill while another app has focus -> AC-006
- A persisted placement value is unknown or misspelled -> AC-007
- The light theme receives a dark system appearance change -> AC-008
- The Swift binary is absent from the build output -> AC-010

## Out of scope
- Any change to `/Users/calibro/Projects/leko-enforcer`, the live LaunchAgent or the daemon on 127.0.0.1:8765.
- Any edit to `src/daemon/`, `src/blocker-extension/`, `src/page-regions/regions.mjs`, `enforcers/`, `voice/audio_io/` or `voice/normalization/`.
- New pass, seal, add-on or enforcement rules, and Windows or Android UI work.
- A settings window, dashboard, menu bar extra or expandable draft card.

## Decisions
- Retire the circle and wide UI by removing `ui/web/`, the `leko-ui` Rust crate and the circle and draft-card `.mjs` tests, while keeping the core voice IPC actions and states (`ptt_start`, `ptt_end`, `idle/listening/held/speaking/thinking/error`) for `runtime.py` and Python voice tests | one visual surface is required and the bridge is tested independently by those retained events | REVERSIBLE
- Implement the pill as a dependency-free Swift and AppKit binary built by `swiftc` | AppKit and Swift 6.1.2 are installed, and the repo has no Swift code yet, so no new Rust crate is needed | REVERSIBLE
- Bridge enforcement events through an atomic JSON event file beside `rules.json` | it matches the native store boundary and needs no socket lifecycle or permissions work | REVERSIBLE
- Trigger dictation from the existing voice action path and keep `voice/session/runtime.py` at 349 lines by adding no code there | protected audio files stay untouched and a thin pill action maps onto existing actions | REVERSIBLE
- Put new headless tests in `tests/ui/`, which has room for two files, and add no file to the full voice test folders | `tests/voice/` and `tests/python/voice/` are already at 8 entries | REVERSIBLE
- Verify with `~/.cargo/bin/cargo build --release && swiftc ui/pill/*.swift -o target/release/leko-pill && ~/.cargo/bin/cargo test && /Users/calibro/.t3/scratch/phone-pilot/talk-venv/bin/python3 -m unittest discover -s tests -p 'test_*.py' -v && node --test tests/ui/*.mjs`, passed to `/contract done --run`; AC-015 keeps only external checks | every component must build and no test may recurse into the full command | REVERSIBLE
