# Contract: native-leko-rewrite

Written before any code. After the user opens it, the contract is locked: criteria cannot be edited to match the code.
Every "<...>" part must be replaced with real text. `/contract open <feature>` refuses a contract that is not complete.

## Goal
Leko Enforcer gets a clean-room native Rust core that stores rules in the canonical 5-pillar format and evaluates them deterministically under an injected clock, with a universal fail-closed add-on verifier hook. This is milestone 1 of a targeted rewrite that drops `.ctsettings` import compatibility and leaves the daemon, extension, enforcers and voice capture layers untouched.

## Domains
Domains: data

## Acceptance criteria
- AC-001: The native model uses only the canonical names: `TechnologyManagementRule`, the six `Target` variants with `WholeDevice { action }` and `DeviceAction::{Lock, Logoff, Restart, Shutdown}`, the three `Schedule` parts, `Penalty`, both `Pass` forms with their three sub-enums, every declared `Seal` variant, and `CustomVerifierSpec { addon_id, sha256, config }`, with snake_case fields and JSON keys and no legacy names.
  - evidence: core/tests/native_vocabulary.rs "every_pillar_type_and_json_key_uses_the_canonical_names"
  - breaker: A legacy name (Door, Lock, BlockSpec, outsideHours, everyOpen, blocked_hours) survives in the new core, or a field is written in camelCase.
- AC-002: A `WholeDevice` rule refuses any penalty and enforces on every unlock, sign-in and boot, while Apps and Websites rules refuse to save without `Some(penalty)`.
  - evidence: core/tests/native_vocabulary.rs "whole_device_rejects_a_penalty_and_apps_and_sites_require_one"
  - breaker: A WholeDevice rule accepts a penalty, or an app rule is saved with no penalty.
- AC-003: Tightening a rule lands immediately, while weakening, pausing or deleting a sealed rule is refused unless the seal is satisfied or the change is queued under `QueuedDelaySeal`.
  - evidence: core/tests/native_seals.rs "tightening_lands_now_and_weakening_waits_for_the_seal"
  - breaker: A weakening, pause or delete slips through a seal, or a tightening is wrongly delayed.
- AC-004: `RoutineSchedule` windows wrap past midnight and follow the supplied timezone, and all schedule evaluation uses the injected clock.
  - evidence: core/tests/native_schedule.rs "windows_cross_midnight_and_follow_the_zone_with_an_injected_clock"
  - breaker: A window that crosses midnight misses an endpoint, or an evaluator reads the real clock.
- AC-005: `usage_based: None` enforces at once with a zero-minute budget during the active `time_based`, and `TimeLimit` and `SessionLimit` reset on their declared period.
  - evidence: core/tests/native_schedule.rs "a_zero_budget_enforces_at_once_and_limits_reset_on_the_period"
  - breaker: A zero budget waits for a positive one, or a usage limit never resets.
- AC-006: Earning a pass never edits the stored rule, and an `EarnPass` honours its requirement, its `TimedPass` or `RestOfDayPass` length and its `LimitedPasses` or `UnlimitedPasses` cap before enforcement resumes.
  - evidence: core/tests/native_passes.rs "an_earned_pass_expires_at_its_length_and_respects_its_cap"
  - breaker: An expired or capped pass keeps bypassing, or earning a pass mutates the rule.
- AC-007: Every declared seal variant is present with its declared fields, `QueuedDelaySeal` can hold one pending edit and be cancelled before it applies, and `CustomVerifier` carries its optional fallback seal.
  - evidence: core/tests/native_seals.rs "each_seal_declaration_matches_and_a_queued_edit_can_be_cancelled"
  - breaker: A seal variant or field is missing, or a cancelled queued edit still applies.
- AC-008: The verifier hook is fail-closed: a missing runner, a SHA-256 mismatch, an invalid nonce or a runtime error each evaluates to `NotSatisfied` and never to `Satisfied`.
  - evidence: core/tests/native_verifier.rs "every_verifier_failure_returns_not_satisfied"
  - breaker: One failure mode returns `Satisfied` and lets a sealed rule be weakened.
- AC-009: Every evaluation consumes a fresh nonce so a replayed attempt is refused, and a sealed add-on cannot be disabled, edited or deleted because its daemon-owned read-only snapshot is pinned at seal time.
  - evidence: core/tests/native_verifier.rs "a_replayed_nonce_or_a_changed_snapshot_is_refused"
  - breaker: A nonce is accepted twice, or an add-on is swapped after sealing to bypass the rule.
- AC-010: The native JSON store is saved atomically, so a crash before the rename leaves the old store readable, and a missing, empty or corrupt store is reported plainly and never silently overwritten.
  - evidence: core/tests/native_store.rs "a_crash_half_way_through_a_save_leaves_the_old_store_readable"
  - breaker: A partial write leaves broken JSON, or a damaged store is overwritten on load.
- AC-011: No pure evaluator reads the clock or timezone directly; only the clock adapter does.
  - evidence: core/tests/native_shape.rs "only_the_clock_adapter_reads_the_time"
  - breaker: A `SystemTime::now()` call inside an evaluator makes tests and devices disagree.
- AC-012: No stub variants or flags exist for `[PLANNED]` features, `shadow_mode` stays false, and `nsfw` is not overridable.
  - evidence: core/tests/native_vocabulary.rs "no_planned_stubs_and_no_overridable_flags"
  - breaker: A planned feature is exposed as a dead variant or flag, or a protected flag is made writable.
- AC-013: Every file and directory this rewrite creates stays under 350 lines and at or below 8 immediate entries, and no new file contains an em-dash.
  - evidence: core/tests/native_shape.rs "new_files_stay_under_350_lines_and_directories_under_8_entries"
  - breaker: A new file reaches 350 lines, a new directory reaches 9 entries, or an em-dash is introduced.
- AC-014: The protected paths are byte-identical to the clean `main` snapshot.
  - evidence: core/tests/native_shape.rs "protected_paths_are_byte_identical_to_main"
  - breaker: The daemon, blocker extension, `regions.mjs`, enforcers, audio capture or normalization is modified.
- AC-015: The rewrite satisfies the behavioural edge cases in `vectors/` and `extension-vectors/` (midnight wrap, timezones, seal tightening versus weakening, pass expiry and caps, whole-device enforcement) without copying any legacy `.ctsettings` key or JSON shape.
  - evidence: core/tests/native_behaviour_vectors.rs "edge_cases_match_the_vectors_without_legacy_json_keys"
  - breaker: A vector case disagrees, or a legacy key such as `outsideHours` is carried into the native format.

## Checklist coverage
- DATA-1 -> criterion:AC-010
- DATA-2 -> criterion:AC-010
- DATA-3 -> out-of-scope: `.ctsettings` import is dropped entirely, so no earlier native store version exists to convert; the version field is reserved for a future migration contract.

## Derived checklist
(not used: Domains is not "none")

## Edge cases
- A missing or empty store file on first run -> AC-010
- A rule carrying an unknown or legacy key such as `outsideHours` -> AC-001
- A save that fails after the temp file is written but before the rename, so the job fails halfway -> AC-010
- A routine window that wraps past midnight across a daylight-saving change -> AC-004
- An add-on binary whose SHA-256 differs from the pin -> AC-008
- A pass that expires at the same moment a usage limit resets -> AC-006
- A `QueuedDelaySeal` edit cancelled one second before it applies -> AC-007
- A `WholeDevice` rule that also carries a penalty -> AC-002

## Out of scope
- The new CLI commands and kebab-case flags (milestone 2, `native-leko-cli`).
- Voice routing changes and the spoken `[PLANNED]` response (milestone 3, `native-leko-voice-routing`).
- The out-of-process add-on runner and declarative recipe execution (milestone 4, `native-leko-addon-runner`); this contract defines only the hook and its fail-closed behaviour.
- Any `.ctsettings` import, conversion or compatibility path.
- All `[PLANNED]` built-ins: `PomodoroSchedule`, `CommunityLinkedApps`, `CommunityLinkedWebsites`, `EngagementPass`, `RestartSeal`.
- Every protected path: `src/daemon/`, `src/blocker-extension/`, `src/page-regions/regions.mjs`, `enforcers/`, `voice/audio_io/`, `voice/normalization/`.
- Windows and Android builds, cross-device sync, and the live daemon on 127.0.0.1:8765.

## Decisions
- Scope this contract to milestone 1 and split CLI, voice routing and the add-on runner into three later contracts | the 349-line and 8-entry caps make one big rewrite unsafe to land at once, and each layer has its own user-visible behaviour | REVERSIBLE
- Treat the 5-pillar vocabulary in the opening request as the source of truth and write only `docs/vocab/USER_VOCABULARY.md` during milestone 1 | neither file exists on cloud main, in any other branch, or anywhere in git history, verified from a fresh clone; the user skipped VOCABULARY_MIGRATION_AUDIT.md because this is a clean-room rewrite | REVERSIBLE
- Use one versioned native JSON store file written through a temp file, fsync and atomic rename | keeps directory entry counts low and gives crash safety without a new dependency | REVERSIBLE
- Pass a `Clock` trait into evaluators, leaving real clock reads in one adapter | tests can pin time and pure logic stays deterministic | REVERSIBLE
- Delete the legacy core source and all 48 legacy core/tests files in milestone 1, and put all M1 integration evidence in exactly 8 files under core/tests/ | Cargo runs Rust integration tests only from core/tests/, the Python root tests/ folder is invisible to it, and the 8-entry cap in AC-013 needs the legacy files gone | REVERSIBLE
- Run milestone 1 tests with `~/.cargo/bin/cargo test -p leko-core` | legacy cli/ still depends on the old core API in M1, so a full workspace test fails to compile; -p builds only the core package and its dependencies; cargo is installed at that path but the folder is not on this shell's PATH | REVERSIBLE
- Run the full workspace with `~/.cargo/bin/cargo test` from milestone 2 onward | by then cli/ is rewritten against the native core and the whole workspace compiles again | REVERSIBLE
