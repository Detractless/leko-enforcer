# Contract: native-leko-cli

Written before any code. After the user opens it, the contract is locked: criteria cannot be edited to match the code.
Every "<...>" part must be replaced with real text. `/contract open <feature>` refuses a contract that is not complete.

## Goal
Leko gets a clean-room native CLI that manages `TechnologyManagementRule` objects through the `leko-core` crate and its atomic native JSON store, using canonical 5-pillar commands and kebab-case flags. It replaces the legacy `cli/` surface, gives Milestone 3 a stable `--json` output contract, and makes the full Cargo workspace compile again.

## Domains
Domains: cli, data

## Acceptance criteria
- AC-001: The CLI exposes exactly ten canonical modifier-plus-noun rule commands `create-rule`, `list-rules`, `show-rule`, `edit-rule`, `delete-rule`, `queue-edit`, `cancel-edit`, `request-pass`, `check-status` and `evaluate-rule`, plus the three infrastructure subcommands `settings`, `agent` and `enforce`, which keep their existing launch-agent and daemon interface while their internals are adapted to `leko-core`. All rule commands use kebab-case flags including `--store`, `--at`, `--json`, `--target`, `--schedule`, `--penalty`, `--pass` and `--seal`.
  - evidence: cli/tests/arguments.rs "only_canonical_rule_and_infrastructure_commands_are_accepted"
  - breaker: A legacy rule command such as `add-block`, `session`, `switch-off` or `status`, or a legacy flag such as `--open-hours`, `--wall` or `--write`, is still accepted; or the `settings`, `agent` or `enforce` interface that the protected launch agent and daemon call is changed.
- AC-002: The CLI talks only to `leko-core` and never imports, converts or reads a `.ctsettings` zip or any legacy format.
  - evidence: cli/tests/guardrails.rs "the_cli_depends_only_on_the_native_core"
  - breaker: A legacy zip reader or a `.ctsettings` conversion path is reintroduced under `cli/`.
- AC-003: `create-rule` writes a valid `TechnologyManagementRule` to the store named by `--store`, including all ten canonical fields, and preserves the native store version.
  - evidence: cli/tests/rule_create_list_show.rs "create_rule_writes_a_complete_canonical_rule_to_the_named_store"
  - breaker: A rule is saved without `created_at` and `updated_at`, or written to a hard-coded default store instead of `--store`.
- AC-004: `list-rules` and `show-rule` print exact Title Case canonical vocabulary (`Target`, `Schedule`, `Penalty`, `Pass`, `Seal` and their variant names) in human output.
  - evidence: cli/tests/rule_create_list_show.rs "list_and_show_print_exact_title_case_vocabulary"
  - breaker: Output uses legacy words such as Door, Lock, Block, Pass as a door name, or lower-case drift such as "technology management rule".
- AC-005: `edit-rule` applies a tightening change immediately, without needing a seal to be satisfied.
  - evidence: cli/tests/rule_edit.rs "a_tightening_edit_lands_immediately"
  - breaker: A tightening edit is delayed by a seal or reported as refused.
- AC-006: `edit-rule` refuses a weakening, pausing or deleting change while a seal is unsatisfied, and applies it once the seal is satisfied.
  - evidence: cli/tests/rule_edit.rs "weakening_pausing_and_deleting_wait_for_the_seal"
  - breaker: A sealed rule is weakened, paused or deleted without the seal being satisfied.
- AC-007: `queue-edit` places one pending edit under `QueuedDelaySeal` and `cancel-edit` removes it before it lands; a cancelled edit never reaches the stored rule.
  - evidence: cli/tests/rule_edit.rs "a_queued_edit_can_be_cancelled_before_it_applies"
  - breaker: A cancelled queued edit still changes the rule, or a queued edit ignores its delay.
- AC-008: `request-pass` records a pass in passing state without changing the stored rule, and honours the rule's requirement, length and cap through the core evaluator.
  - evidence: cli/tests/pass_request.rs "requesting_a_pass_never_mutates_the_stored_rule"
  - breaker: Earning a pass rewrites the rule's `pass` field, or an expired or capped pass keeps bypassing.
- AC-009: `check-status` and `evaluate-rule` are deterministic under an explicit `--store` path and an injected `--at` timestamp, and report enforcement plus the exact reason from the core decision.
  - evidence: cli/tests/status_evaluate.rs "status_and_evaluation_are_deterministic_under_an_injected_timestamp"
  - breaker: A command reads the real clock, so the same store and timestamp produce different answers on two runs.
- AC-010: `--json` produces a stable machine-readable object whose top-level keys are the canonical field names and whose pillar objects are `target`, `schedule`, `penalty`, `pass` and `seal`, with no prose mixed in.
  - evidence: cli/tests/output_modes.rs "json_output_is_a_stable_canonical_object_for_milestone_three"
  - breaker: A human sentence is printed into stdout JSON, or a key is renamed so Milestone 3 routing cannot parse it.
- AC-011: Wrong or missing arguments print a usage line to stderr and exit non-zero; results go to stdout and errors to stderr with distinct exit codes.
  - evidence: cli/tests/arguments.rs "wrong_or_missing_arguments_print_usage_to_stderr_and_exit_non_zero"
  - breaker: A bad invocation exits zero, or an error message is printed to stdout where a caller expects JSON.
- AC-012: Running a create or request command twice on empty or unchanged input is safe, idempotent and says what happened.
  - evidence: cli/tests/arguments.rs "repeated_and_empty_input_commands_are_safe_and_idempotent"
  - breaker: A second create duplicates the rule, or an empty store crashes the command.
- AC-013: A missing, empty or corrupt `--store` file is reported plainly and is never overwritten, and an interrupted save leaves the previous store readable.
  - evidence: cli/tests/store_atomic.rs "a_broken_store_is_reported_and_a_partial_save_keeps_the_old_one"
  - breaker: A corrupt store is silently replaced with an empty one, or a half-written file is left as the live store.
- AC-014: The full Cargo workspace compiles and all tests pass with `~/.cargo/bin/cargo test`, covering both `leko-core` and `leko-cli`.
  - evidence: cli/tests/guardrails.rs "the_full_workspace_builds_with_the_native_core"
  - breaker: Legacy `cli/` code still references the old core API, so the workspace fails to compile after the core rewrite.
- AC-015: Every new or rewritten file is under 350 lines, every rewritten directory has at most 8 immediate entries including `cli/tests/`, no file contains an em-dash, protected paths stay byte-identical to `main`, and `shadow_mode` and `nsfw` remain fixed or non-overridable.
  - evidence: cli/tests/guardrails.rs "shape_limits_protected_paths_and_fixed_flags_hold"
  - breaker: `cli/tests/` grows past 8 files, an em-dash is introduced, or a protected daemon or extension file is modified.

## Checklist coverage
- CLI-1 -> criterion:AC-011
- CLI-2 -> criterion:AC-011
- CLI-3 -> criterion:AC-012
- DATA-1 -> criterion:AC-013
- DATA-2 -> criterion:AC-013
- DATA-3 -> out-of-scope: `.ctsettings` import is dropped and no earlier native store version exists; the version field stays reserved for a future migration contract.

## Derived checklist
(not used: Domains is not "none")

## Edge cases
- An empty store with no rules on first run -> AC-012
- A legacy command or legacy flag such as `add-block` or `--write` -> AC-001
- A save that fails after the temp file is written but before the rename, so the job fails halfway -> AC-013
- A `--at` timestamp earlier than the rule's `created_at` -> AC-009
- A `QueuedDelaySeal` edit cancelled one second before it would apply -> AC-007
- A `CustomVerifier` pass whose add-on binary is missing or whose SHA-256 does not match -> AC-008
- A rule name containing quotes, non-ASCII letters and a newline, rendered in both output modes -> AC-010
- A `WholeDevice` rule submitted with `--penalty` -> AC-003

## Out of scope
- Any change to `voice/routing/`, including the spoken `[PLANNED]` response (milestone 3, `native-leko-voice-routing`).
- The out-of-process add-on runner and declarative recipe execution (milestone 4, `native-leko-addon-runner`); M2 only exposes the `CustomVerifier` spec fields.
- Legacy rule commands and flags, `.ctsettings` import, and compatibility shims for the old rule CLI; the `settings`, `agent` and `enforce` infrastructure subcommands are kept with their interfaces preserved, as stated in AC-001.
- All `[PLANNED]` built-ins: `PomodoroSchedule`, `CommunityLinkedApps`, `CommunityLinkedWebsites`, `EngagementPass`, `RestartSeal`.
- Every protected path: `src/daemon/`, `src/blocker-extension/`, `src/page-regions/regions.mjs`, `enforcers/`, `voice/audio_io/`, `voice/normalization/`, and the live daemon on 127.0.0.1:8765.
- Windows and Android builds, cross-device sync, and the native UI.

## Decisions
- Replace the legacy rule-command source and all 97 legacy `cli/tests/` files in M2 rather than adapting them, while adapting `settings`, `agent` and `enforce` internally to `leko-core` with their launch-agent and daemon interface preserved | the rule commands target the removed core API and would leave directories far past the 8-entry cap, while the three infrastructure subcommands back protected plumbing | REVERSIBLE
- Put all M2 integration evidence in exactly 8 files under `cli/tests/` | Cargo runs Rust integration tests only from `cli/tests/` and the 8-entry cap is checked in AC-015 | REVERSIBLE
- Keep the CLI a single binary named `leko` that calls `leko_core` public functions directly | no duplicate rule types and no second source of truth | REVERSIBLE
- Expose the three infrastructure subcommands `settings`, `agent` and `enforce` alongside the ten canonical rule commands, preserving their launch-agent and daemon interface and adapting only their internals to `leko-core` | the protected enforcement plumbing calls them, so changing their interface would break work outside this contract | REVERSIBLE
- Inject time through an explicit `--at` RFC3339 flag parsed into a `Moment`, leaving real clock reads in one CLI adapter | tests stay deterministic and the core remains pure | REVERSIBLE
- Verify with `~/.cargo/bin/cargo test` from M2 onward | the rewritten `cli/` restores workspace compilation, which the M1 crate-only command could not | REVERSIBLE
