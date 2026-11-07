# Contract: macos-launch-agent

Draft for review. After the user opens it, the criteria are locked.

## Goal
Provide a per-user macOS LaunchAgent that starts Leko app enforcement at login with an explicitly selected settings zip, and commands to inspect and remove that agent. Installation is an explicit action; drafting and building this contract do not install or start it.

## Domains
Domains: none

## Acceptance criteria
- AC-001: An install command accepts absolute paths to the Leko executable and settings zip, checks that the executable is runnable and the zip and local time zone are valid, and previews the exact configuration without changing disk or launchd unless `--write` is supplied.
  - evidence: cli/tests/launch_agent_install.rs "install_previews_and_checks_its_inputs"
  - breaker: A missing file installs a broken service, or a preview writes files or starts a process.
- AC-002: With `--write`, installation writes a valid per-user LaunchAgent plist under the current user's `~/Library/LaunchAgents/`, with a Leko-specific label, separate argument array entries for `enforce` and the exact settings path, login startup, and bounded restart behavior; paths containing spaces and XML characters round-trip correctly.
  - evidence: cli/tests/launch_agent_plist.rs "plist_preserves_paths_and_agent_settings"
  - breaker: A path is split or interpreted as shell syntax, another user's launch domain is targeted, or a restart loop runs without a limit.
- AC-003: Installing an identical agent twice is safe; an unrelated existing plist at the target path is refused and left untouched. Installation failures report which step failed and do not claim the agent is running.
  - evidence: cli/tests/launch_agent_install.rs "install_is_idempotent_and_refuses_unrelated_files"
  - breaker: Installation overwrites another service or claims success after launchd refused it.
- AC-004: An installed agent is registered in the current user's GUI launchd domain and is configured to start on login; failure to reach that domain is reported without claiming activation.
  - evidence: cli/tests/launch_agent_control.rs "registration_uses_the_current_gui_domain"
  - breaker: The service is loaded into a root or another user's domain, or registration failure is hidden.
- AC-005: A status command reports separately whether Leko's plist exists, whether launchd has the service loaded, and whether it currently has a running process; missing, stopped and launchd-error cases are distinguished.
  - evidence: cli/tests/launch_agent_status.rs "status_distinguishes_installed_loaded_and_running"
  - breaker: A plist on disk is reported as proof that enforcement is running.
- AC-006: An uninstall command previews its effect by default; with `--write`, it unloads only Leko's own agent and removes only its own recognized plist, leaving the settings zip and unrelated services untouched.
  - evidence: cli/tests/launch_agent_uninstall.rs "uninstall_only_removes_the_owned_agent"
  - breaker: A preview stops enforcement, a settings zip is removed, or an unrelated agent is unloaded or deleted.
- AC-007: Restarting after a process exit is throttled so an invalid configuration cannot create a tight launch loop; errors remain available to the user without including settings contents.
  - evidence: cli/tests/launch_agent_plist.rs "restart_is_throttled_and_errors_are_visible"
  - breaker: A bad settings file causes unbounded rapid restarts, or logs expose the contents of the zip.
- AC-008: A real Mac integration check uses an isolated test agent and disposable app to show launchd starting enforcement and stopping it on cleanup, without installing or removing the user's real agent.
  - evidence: docs/macos-launch-agent-check.md "Real Mac check"
  - breaker: Plist and mocked tests pass while launchd never starts the real enforcement process.
- AC-009: Creating, checking, or removing the agent does not modify Leko's block rules, waiting changes, existing agents belonging to other apps, or system-wide LaunchAgents.
  - evidence: cli/tests/launch_agent_uninstall.rs "settings_and_unrelated_agents_are_unchanged"
  - breaker: Removing or replacing the service also deletes settings or alters another application's startup file.

## Checklist coverage
Not used: Domains is none; the derived checklist below stands in for it.

## Derived checklist
- D-1: Preview makes no changes and checks inputs -> criterion:AC-001
- D-2: The plist preserves paths and starts the intended command -> criterion:AC-002
- D-3: Existing unrelated files are protected -> criterion:AC-003
- D-4: Registration uses the current user's GUI session -> criterion:AC-004
- D-5: Status distinguishes installed from actually running -> criterion:AC-005
- D-6: Uninstall is explicit and preserves settings -> criterion:AC-006
- D-7: Failed processes are not restarted in a tight loop -> criterion:AC-007
- D-8: The real launchd path is exercised with a disposable agent -> criterion:AC-008
- D-9: Other apps' agents and Leko settings remain unchanged -> criterion:AC-009

## Edge cases
- Paths containing spaces, ampersands, quotation marks and non-ASCII characters -> AC-002
- Settings zip removed or corrupted after installation, or the executable moved -> AC-007
- Another plist already occupies the intended path -> AC-003
- launchd rejects registration -> AC-004
- A service is loaded but not running, or a plist exists but is not loaded -> AC-005
- Repeated install and removal -> AC-003
- A test agent left over after a failed test -> AC-008

## Out of scope
- Root-owned services, system-wide installation, tamper resistance, protection from an administrator, and any promise that enforcement cannot be stopped by the user.
- A signed installer, menu-bar UI, automatic software updates, authenticated IPC, browser blocking and phone sync.
- Automatically settling pending changes or changing how block decisions are made.
- Actually installing the persistent agent as part of drafting, implementation, or tests; the real check uses a disposable agent and cleans it up.

## Decisions
- Use a per-user LaunchAgent, not a root daemon | app enforcement needs the logged-in user's GUI session; stronger protection is planned for Phase 7 | REVERSIBLE
- Use explicit absolute executable and settings paths | avoids guessing which rules to enforce and keeps launchd arguments unambiguous | REVERSIBLE
- Preview by default and require `--write` for install and removal | makes persistent startup changes deliberate and reviewable | REVERSIBLE
- Use a Leko-specific label and isolated test label | avoids using Cold Turkey's identity or touching the user's real agent in a test | REVERSIBLE
- Throttle restarts and report failures | prevents a tight restart loop and avoids representing a stopped service as active | REVERSIBLE
