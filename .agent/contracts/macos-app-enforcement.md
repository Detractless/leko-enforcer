# Contract: macos-app-enforcement

Draft for review. After the user opens it, the criteria are locked.

## Goal
Run a user-started macOS enforcement process that reads an existing settings zip and terminates a running app when a supported rule closes its bundle ID. Demonstrate a scheduled block on a real Mac.

## Domains
Domains: none

## Acceptance criteria
- AC-001: The process reads a specified settings zip and uses the existing core decision with the current moment, local time zone, and each running app's bundle ID.
  - evidence: cli/tests/enforcement_decisions.rs "running_apps_are_judged_against_settings"
  - breaker: The process closes an app that the core says is open, or uses the wrong zone or bundle ID.
- AC-002: A supported closed rule requests termination of every matching running instance, including an instance launched after the process starts; open or uncovered apps are not terminated.
  - evidence: cli/tests/enforcement_decisions.rs "only_closed_running_apps_are_terminated"
  - breaker: A later launch escapes the next tick, or an unrelated app is terminated.
- AC-003: The process checks at least once per second while running and re-evaluates schedule and session boundaries, including after sleep or a clock change.
  - evidence: cli/tests/enforcement_loop.rs "each_tick_uses_the_current_time_and_apps"
  - breaker: An app remains open because a decision was cached across a schedule boundary.
- AC-004: The process re-reads changed settings and applies valid changes without restart; an unreadable or invalid replacement is reported and the last valid settings remain in effect.
  - evidence: cli/tests/enforcement_loop.rs "invalid_reload_keeps_last_valid_settings"
  - breaker: A corrupt replacement silently disables an active block.
- AC-005: A missing or invalid initial settings file or unavailable local time zone stops startup with a clear error; no UTC fallback is guessed.
  - evidence: cli/tests/enforcement_startup.rs "invalid_startup_inputs_stop_enforcement"
  - breaker: The process starts with empty settings or a guessed time zone.
- AC-006: Apps without a bundle ID are skipped and reported; the enforcement process and apps with bundle IDs `com.apple.finder`, `com.apple.dock`, `com.apple.loginwindow` and `com.apple.systemuiserver` are never terminated, including under all-apps-except coverage.
  - evidence: cli/tests/enforcement_decisions.rs "essential_and_unidentified_apps_are_not_terminated"
  - breaker: An all-apps-except rule terminates an essential app or the blocker itself.
- AC-007: A failed termination is reported with the app identity and retried on a later tick while the rule remains closed; a failure for one app does not prevent checking others.
  - evidence: cli/tests/enforcement_loop.rs "termination_failure_is_reported_and_retried"
  - breaker: One failed termination stops the loop or is silently treated as successful.
- AC-008: Rules requiring usage counts are reported as not judged; they are not represented as enforced by this feature.
  - evidence: cli/tests/enforcement_decisions.rs "unsupported_usage_rules_are_reported"
  - breaker: A minutes-only rule is presented as enforced when no usage data exists.
- AC-009: On a real Mac, a documented manual check demonstrates that a disposable test app is terminated while its scheduled rule is closed and remains running when the rule is open.
  - evidence: docs/macos-enforcement-check.md "Real Mac check"
  - breaker: Tests pass with a fake app list but the actual macOS integration never closes the test app.

## Checklist coverage
Not used: Domains is none; the derived checklist below stands in for it.

## Derived checklist
- D-1: Decisions use current settings, time, zone and bundle IDs -> criterion:AC-001
- D-2: Only covered, closed apps are terminated -> criterion:AC-002
- D-3: Schedule boundaries and later launches are checked -> criterion:AC-003
- D-4: Bad replacement settings do not erase the last valid rules -> criterion:AC-004
- D-5: Invalid startup inputs fail visibly -> criterion:AC-005
- D-6: Essential and unidentified apps are protected -> criterion:AC-006
- D-7: Termination failures are visible and retried -> criterion:AC-007
- D-8: Unsupported rules are reported honestly -> criterion:AC-008
- D-9: The real macOS path is demonstrated -> criterion:AC-009

## Edge cases
- Multiple instances of one bundle ID, an app launched after startup, and an uncovered app -> AC-002
- A session ending or a schedule changing while the process is running, including after sleep -> AC-003
- Settings become unreadable after a valid load -> AC-004
- A running process has no bundle ID or is an essential app -> AC-006
- An app refuses termination while another covered app is running -> AC-007

## Out of scope
- Automatic launch at login, a LaunchAgent installer, root helper, watchdog, tamper resistance and authenticated IPC.
- Website blocking, browser extensions, usage accounting, doors and passes, voice, UI and phone sync.
- Guaranteeing termination of apps that refuse the macOS request or lack a bundle ID.

## Decisions
- Start manually from the CLI for this first slice | makes the enforcement path testable before installing a persistent service | REVERSIBLE
- Poll running apps and current time every second | matches the original Phase 2 plan and detects later launches and wakeups | REVERSIBLE
- Keep the last valid settings after a failed reload | avoids silently dropping an already loaded block | REVERSIBLE
- Protect Finder, Dock, login window, SystemUIServer and the enforcement process; skip apps without a bundle ID | protects basic desktop access and the enforcement process; this is Leko's proposed policy, not a claim about Cold Turkey | REVERSIBLE
- Request normal macOS app termination and report failures | keeps the first integration bounded and makes its limits visible | REVERSIBLE

## Reference review
- The saved Swift reconstruction, `work/reconstruction-stage/Sources/ColdTurkeyEngine/EngineBlocksApps.swift`, describes matching apps by bundle ID and also matching executable names and folders. It uses terminate followed by force terminate for ordinary app matches; minimize mode hides the frontmost app. For Finder app matches it hides Finder, while a blocked folder open in Finder can cause termination. These are reconstructed static findings, not a new runtime observation.
- The reconstruction's `EngineTimers.swift` calls the app-blocking tick on a five-second timer; a separate engine timer runs every second. Leko's proposed one-second app check is its own design choice.
- Finder, screen saver and login window are excluded from one statistics counter in the reconstruction. That is not evidence of a general enforcement exemption list. The reviewed app-blocking function does not establish a comprehensive Cold Turkey essential-app list.
- The initial Leko slice intentionally uses bundle IDs only and requests normal termination. Force termination, file and folder matching, and Finder-specific blocking remain outside this draft.

## Accepted timing exception
- On 2026-11-06, the user accepted a 900 ms target with reported overruns in place of AC-003's strict one-second maximum. The original criterion above is preserved as written; the strict maximum remains unmet because scheduling and slow operations can delay a check.
- Forward and backward clock changes and a delayed wakeup were simulated in `cli/tests/enforcement_loop.rs`. Actual Mac sleep and system-clock changes have not been tested.
