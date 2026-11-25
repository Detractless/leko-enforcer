# Contract: native-leko-addon-runner

Written before any code. After the user opens it, the contract is locked: criteria cannot be edited to match the code.
Every "<...>" part must be replaced with real text. `/contract open <feature>` refuses a contract that is not complete.

## Goal
Leko gets a concrete tamper-proof verifier runner behind the fail-closed `CustomVerifier` hook, supporting out-of-process add-ons and built-in no-code recipes for both `Pass` and `Seal`. A pinned add-on is copied into a configurable read-only quarantine, and every failure evaluates to `NotSatisfied`.

## Domains
Domains: cli, data

## Acceptance criteria
- AC-001: The same runner evaluates `Pass::EarnPass` with `CustomVerifier` and `Seal::CustomVerifier`, and a missing runner still yields `NotSatisfied` for both.
  - evidence: core/src/services/verifier.rs "both_pass_and_seal_share_the_fail_closed_hook"
  - breaker: A pass bypasses the hook, or a seal accepts a result the pass path would reject.
- AC-002: The out-of-process challenge protocol spawns the pinned executable with a timeout, writes JSON containing `challenge_id`, a fresh 256-bit `nonce` and `config` to stdin, and accepts only a JSON stdout reply with the same `challenge_id`, the same unused `nonce` and `status: "satisfied"`.
  - evidence: core/src/services/runner.rs "challenge_protocol_requires_exact_fields"
  - breaker: A stale challenge id, a different nonce or `status: "rejected"` is treated as satisfied.
- AC-003: At pin time the runner hashes the manifest, payload and exact config, then copies the verified bundle into a read-only quarantine under a configurable snapshot root.
  - evidence: core/tests/native_verifier.rs "pin_copies_a_hashed_bundle_into_the_snapshot_root"
  - breaker: The pinned hash is taken from a different config than the one copied, or the snapshot root is hard-coded.
- AC-004: Editing, disabling or deleting the user-facing add-on after pinning cannot change the pinned snapshot, while tampering with the snapshot itself produces a hash mismatch and `NotSatisfied`.
  - evidence: core/tests/native_verifier.rs "source_edits_cannot_change_a_pinned_snapshot"
  - breaker: A source edit silently changes a sealed verifier, or a mutated snapshot still verifies.
- AC-005: Missing add-on or snapshot, hash mismatch, timeout, non-zero exit, malformed JSON, wrong or replayed nonce and runtime error each evaluate to `NotSatisfied` and never to `Satisfied`.
  - evidence: core/src/services/runner.rs "every_failure_mode_returns_not_satisfied"
  - breaker: One failure path returns `Satisfied` and lets a sealed rule be weakened.
- AC-006: `Seal::CustomVerifier` uses `fallback_seal` when it is present and the verifier fails, and stays hard-locked when `fallback_seal` is `None`.
  - evidence: core/src/services/verifier.rs "fallback_seal_is_used_only_when_present"
  - breaker: A missing fallback silently allows the edit, or a present fallback is ignored.
- AC-007: Every challenge nonce is 256 bits of randomness and single use, and a replayed nonce is refused even within the same process.
  - evidence: core/src/services/nonce.rs "nonces_are_256_bit_and_single_use"
  - breaker: A counter-based or short nonce is reused, so a captured reply can replay.
- AC-008: Built-in no-code recipes execute inside the runner for command exit code plus expected token, local HTTP JSON field threshold, and structured prompt or typing check, without a compiled add-on.
  - evidence: core/src/services/recipe.rs "all_three_builtin_recipes_run_without_code"
  - breaker: A simple check requires a custom executable, or a recipe can return `Satisfied` on a failed command.
- AC-009: The CLI exposes canonical add-on commands and flags to pin, list and verify add-ons, and `request-pass` and `edit-rule` work against pinned verifiers while `--json` fields remain stable.
  - evidence: cli/src/services/addon.rs "canonical_addon_commands_and_stable_json"
  - breaker: A new command uses a legacy name or flag, or an add-on result changes a `--json` key.
- AC-010: A missing or wrong add-on argument prints a usage line to stderr and exits non-zero; results go to stdout and errors to stderr.
  - evidence: cli/src/services/addon.rs "usage_and_streams_follow_the_cli_contract"
  - breaker: A bad invocation exits zero, or an error is printed into the JSON stream.
- AC-011: Pinning or verifying twice on unchanged input is safe and idempotent, and an empty snapshot root says what happened rather than crashing.
  - evidence: cli/src/services/addon.rs "repeated_and_empty_addon_requests_are_safe"
  - breaker: A second pin duplicates the snapshot, or an empty root panics.
- AC-012: A pin interrupted after the temp file is written leaves the previous snapshot readable and unchanged.
  - evidence: core/tests/native_verifier.rs "a_partial_pin_write_keeps_the_previous_snapshot"
  - breaker: A half-written snapshot becomes the live pinned copy.
- AC-013: A missing, empty or corrupt add-on manifest is reported plainly with the path named, and nothing is overwritten.
  - evidence: core/tests/native_verifier.rs "a_broken_manifest_is_reported_without_overwriting"
  - breaker: A corrupt manifest is silently replaced with an empty one.
- AC-014: A snapshot written in an unknown version is refused and the version field is preserved for later migration.
  - evidence: core/tests/native_verifier.rs "an_unknown_snapshot_version_is_refused"
  - breaker: A future snapshot is silently treated as the current format.
- AC-015: A Python test checks the guardrails (350-line cap, 8-entry cap, zero em-dashes, byte-identical protected paths, no `/var/lib/leko` or live daemon touches) and exercises the add-on CLI end to end; the full suite command is recorded in Decisions and run with `/contract done --run`.
  - evidence: tests/python/cli/test_addon_runner.py "guardrails_and_addon_cli_end_to_end_hold"
  - breaker: A test writes outside a temporary directory, a directory grows past 8 entries, a protected file changes, or an add-on command cannot be called from Python.

## Checklist coverage
- CLI-1 -> criterion:AC-010
- CLI-2 -> criterion:AC-010
- CLI-3 -> criterion:AC-011
- DATA-1 -> criterion:AC-012
- DATA-2 -> criterion:AC-013
- DATA-3 -> criterion:AC-014

## Derived checklist
(not used: Domains is not "none")

## Edge cases
- An empty snapshot root and no pinned add-on -> AC-011
- A wrong `challenge_id` in the reply -> AC-002
- The add-on writes valid JSON, then exits non-zero, so evaluation fails halfway -> AC-005
- The add-on process exceeds its timeout -> AC-005
- A `Seal::CustomVerifier` with no fallback and a missing binary -> AC-006
- The same nonce presented twice in one session -> AC-007
- A recipe whose HTTP endpoint is a local mock returning a missing field -> AC-008
- A snapshot manifest stamped with a future version -> AC-014

## Out of scope
- Any `.ctsettings` import, legacy CLI shim or change to M1, M2 and M3 behavior beyond the add-on surface.
- Any edit to `src/daemon/`, `src/blocker-extension/`, `src/page-regions/regions.mjs`, `enforcers/`, `voice/audio_io/` or `voice/normalization/`.
- New built-in `[PLANNED]` features, platform enforcement changes, cross-device sync and the native UI.
- Signing the add-on with a private key or distributing a catalog of community add-ons.

## Decisions
- Keep the runner inside `leko-core/src/services/` and expose it through the existing `VerifierHook` trait | the hook and fail-closed rules already live there and the folder has room for five more files | REVERSIBLE
- Generate nonces with a 256-bit random source from a small dependency rather than a process counter | a captured reply must not be replayable by guessing the next value | REVERSIBLE
- Support both JSON and TOML recipe templates behind one parsed `Recipe` type | users can write simple checks without compiling code, and both formats are common for local config | REVERSIBLE
- Make the snapshot root a CLI flag with an environment fallback, defaulting to a directory under the user's own Leko data folder | tests need temporary roots and must never write to `/var/lib/leko` | REVERSIBLE
- Put M4 runner and recipe unit tests inside the new service files, route snapshot-pin tests into the existing `core/tests/native_verifier.rs`, and put the CLI integration test in `tests/python/cli/` | `core/tests/` and `cli/tests/` already have 8 entries each, and editing an existing test file keeps them at 8 | REVERSIBLE
- Verify the whole change with `~/.cargo/bin/cargo test && /Users/calibro/.t3/scratch/phone-pilot/talk-venv/bin/python3 -m unittest discover -s tests -p 'test_*.py' -v`, passed to `/contract done --run`; AC-015 keeps only the non-recursive Python guardrail and end-to-end check | the full command must not be invoked from inside a discovered test, and the talk-venv interpreter has the dependencies the suite needs | REVERSIBLE
