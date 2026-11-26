# native-leko-usage-tracker decisions

Nothing is locked until the user types `/contract open native-leko-usage-tracker`. These are the choices only the user can make, each with one recommended answer.

1. **Foreground & browser site observation source:**
   - Recommended: Add an injectable foreground-observer trait (or method on `OsExecutor`) returning frontmost bundle ID and optional active site. On macOS, read `NSWorkspace.frontmostApplication` and active browser domain from existing read-only heartbeat JSON (`--heartbeats`, optional `#[serde(default)] active_site`) or daemon evaluation logs (`data/runtime/evaluations.jsonl`), with zero edits to protected `src/daemon/` or `src/blocker-extension/`.
   - Alternative: Query live Chrome/Safari via AppleScript on every tick, which triggers macOS Automation prompts and fails in headless CI.

2. **Usage state file & second-level `Time Limit` precision:**
   - Recommended: Persist tracked usage in a separate atomic sibling `.usage-state.json` (`rules.usage-state.json` beside `rules.json`, version 1) storing `period_key`, `active_seconds`, `sessions`, `last_seen_at`, `away_since`, and `engagement_seconds`. Evaluate `TimeLimit { minutes }` using `active_seconds < minutes * 60` so enforcement triggers at the exact minute boundary and `leko-pill` gets `MM:SS` resolution.
   - Alternative: Store whole-minute integers only, which allows up to 59 extra seconds and cannot drive an `MM:SS` timer display.

3. **Session boundary & Cmd-Tab flicker debounce:**
   - Recommended: Default debounce gap of 5 seconds (overridable via `--session-debounce-seconds`). Leaving a target and returning within 5 seconds continues the existing session; leaving for at least 5 seconds closes the session so the next entry increments `sessions`. Under `SessionLimit { max_sessions }`, sessions `1..=max_sessions` stay allowed while continuously active and block on re-entry beyond `max_sessions`.
   - Alternative: Count every focus change immediately, which burns a full daily budget on accidental Cmd-Tab switches.

4. **`Engagement Pass` CLI & target focus schema:**
   - Recommended: Add `PassRequirement::EngagementPass { targets: Vec<String>, required_seconds: u64 }` in `leko-core`, configurable via `--engagement-pass` (plus `--engagement-targets` / `--engagement-seconds` or `--pass` JSON) in `create-rule` and `request-pass`. `leko enforce` accumulates focus seconds in `usage-state.json` and auto-grants the rule's `PassLength` into `rules.pass-state.json` once satisfied (respecting `PassCap`), while `request-pass --elapsed-seconds N` supports instant deterministic CLI testing.
   - Alternative: Require a separate blocking foreground wait inside `request-pass`, which would stall CLI and voice invocations.

5. **`leko-pill` `quietTimer` countdown bridge & module layout:**
   - Recommended: Put pill timer polling into a new `ui/src/timer_poll.rs` helper (keeping `ui/src/main.rs` below 350 lines) that reads active pass remaining seconds plus an atomic `usage_timer` field in `pill-events.json` emitted by `leko enforce`, sending `timer_start` while inside an active `Time Limit` target or `TimedPass` and `timer_stop` when none is active.
   - Alternative: Have `leko-pill` re-implement rule schedule and foreground matching in the UI binary, duplicating `leko-core` logic.

6. **Directory cap test placement:**
   - Recommended: Add the three allowed new implementation modules (`core/src/services/usage_state.rs`, `cli/src/services/usage_tracker.rs`, `ui/src/timer_poll.rs`) and place M9 tests into existing files inside the 8-entry capped test directories (`core/tests/`, `cli/tests/`, `tests/python/cli/`, `tests/python/routing/`, `tests/ui/`).
   - Alternative: Refactor test subdirectories into deeper hierarchies mid-milestone, adding unnecessary churn.

Verification: `~/.cargo/bin/cargo build --release && ~/.cargo/bin/cargo test && /Users/calibro/.t3/scratch/phone-pilot/talk-venv/bin/python3 -m unittest discover -s tests -p 'test_*.py' -v && node --test tests/ui/*.mjs`, passed to `/contract done --run`.

Branch: `feat/m9-usage-tracker`, created off clean `main` at `bd7f7af`. No code or live service, plist, binary, or old checkout was modified for this draft.
