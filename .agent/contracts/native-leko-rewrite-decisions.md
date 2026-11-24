# native-leko-rewrite decisions

Resolved: the user accepted all four recommended answers, with one change to decision 2. Nothing is locked until the user types `/contract open native-leko-rewrite`.

1. **Scope:** Accepted. Four sequential contracts. This contract is M1 (native core model, deterministic evaluators, native store, verifier hook), then `native-leko-cli` (M2), `native-leko-voice-routing` (M3), `native-leko-addon-runner` (M4).

2. **Vocabulary docs:** Accepted with change. Create only `docs/vocab/USER_VOCABULARY.md` from the user's 5-pillar vocabulary in M1. Skip `VOCABULARY_MIGRATION_AUDIT.md` because this is a clean-room rewrite. Neither file exists on cloud main, in any other branch, or in git history.

3. **Store layout:** Accepted. One versioned JSON store file, written to a temp file, fsynced, then atomically renamed.

4. **Voice `[PLANNED]` handling:** Accepted. Speak one fixed "not available yet" line and make no CLI call. M3 routing depends on this.

Milestone map: M1 `native-leko-rewrite` (core model, evaluators, store, verifier hook); M2 `native-leko-cli`; M3 `native-leko-voice-routing`; M4 `native-leko-addon-runner`.

Draft fixes requested before open: M1 Rust integration evidence lives in exactly 8 files under `core/tests/`, including the protected-paths check moved out of the Python root `tests/` folder; the M1 test command is `~/.cargo/bin/cargo test -p leko-core` so the unrewritten `cli/` crate does not block compilation.
