# native-leko-countdowns-and-voice decisions

Nothing is locked until the user types `/contract open native-leko-countdowns-and-voice`. These are the choices only the user can make, each with one recommended answer.

1. **Unified Countdowns Data Source & Command Shape:**
   - Recommended: Add a pure `core/src/countdowns.rs` module and dispatch `leko countdowns` through `cli/src/commands/check_status.rs` (keeping `core/src/domain/` and `cli/src/commands/` within the 8-file cap). Include active `Time Limit`, active `Timed Pass`, active `Engagement Pass` (when focus > 0s), upcoming `Routine Schedule` / `One Time Schedule` transitions, and pending `Queued Delay Seal` maturity, sorted by `remaining_seconds` ascending then urgency (`Time Limit > Timed Pass > Engagement Pass > Schedule > Queued Delay Seal`).
   - Alternative: Query five separate CLI commands (`list-rules`, `check-status`, `show-rule`, etc.) on every pill tick and voice turn, which adds process overhead and risks inconsistent sorting.

2. **`timerThresholdMinutes` & Pill Timer Sourcing:**
   - Recommended: Add `timerThresholdMinutes` (default `15`) alongside `quietTimer` in `pill-config.json`. `leko enforce` writes the soonest qualifying countdown to `pill-events.json`, and `ui/src/timer_poll.rs` shows `MM:SS` only when `seconds <= timerThresholdMinutes * 60`, otherwise emitting `timer_stop` so the pill returns cleanly to the idle dash.
   - Alternative: Always display the nearest countdown regardless of horizon, which fills the idle pill with multi-hour schedule timers all day.

3. **Expandable Monochrome Reply Card Layout & Resize Behavior:**
   - Recommended: Add `ui/src/card.rs` and `ui/src/web/card.css` to render a compact monochrome rounded rectangle below/around the pill when Maeve emits a `reply_card` IPC event (typed text + optional 2-to-3 column table), pausing auto-collapse on card hover and closing after speaking finishes, on outside click, or on `Escape`. Never resize the OS window on idle capsule hover (`Blocked` always preempts).
   - Alternative: Resize the `NSPanel` dynamically on idle capsule hover, which introduces macOS window-server jitter.

4. **LLM Disconnection & Deterministic Voice Routing:**
   - Recommended: Add `voice/routing/deterministic_router.py` (within the 8-file cap in `voice/routing/`) and make it the sole default router in `voice/session/query_flow.py`, gating legacy LFM calls strictly behind `LEKO_ENABLE_LLM_ROUTER=1` (off by default) so zero LLM calls or server launches happen during normal voice turns.
   - Alternative: Keep LFM as a fallback for unrecognized queries, which reintroduces latency and hallucinated responses when speech transcripts miss a phrase.

5. **File & Test Placement Under 8-Entry / 350-Line Caps:**
   - Recommended: Keep `voice/session/runtime.py`, `voice/core/assistant.py`, and `voice/session/query_flow.py` at or below 349 lines by placing new routing/templates in `voice/routing/deterministic_router.py` and adding new tests to existing test files in `core/tests/`, `cli/tests/`, `tests/python/routing/`, `tests/python/session/`, `tests/python/cli/`, and `tests/ui/`.
   - Alternative: Add new files into already-full 8-file directories (`tests/python/routing/`, `core/src/domain/`, `cli/src/commands/`), which fails `check_structure.py`.

Verification: `~/.cargo/bin/cargo build --release && ~/.cargo/bin/cargo test && /Users/calibro/.t3/scratch/phone-pilot/talk-venv/bin/python3 -m unittest discover -s tests -p 'test_*.py' -v && node --test tests/ui/*.mjs`, passed to `/contract done --run`.

Branch: `feat/countdowns-card-deterministic-voice`, created off clean `main` at `09bd0eb`. No code or live path was modified for this draft.
