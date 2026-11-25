# native-leko-addon-runner decisions

Resolved: the user approved all five recommended answers. Nothing is locked until the user types `/contract open native-leko-addon-runner`.

1. **Runner location:** Approved. Implement the runner inside `leko-core/src/services/` behind the existing `VerifierHook` trait, with the CLI as a thin caller.

2. **Nonce source:** Approved. Use 256 bits of real randomness, single use per challenge. A captured reply must not be replayable by guessing the next value.

3. **Recipe formats:** Approved. Accept both JSON and TOML templates through one parsed `Recipe` type so fail-closed behavior is identical.

4. **Snapshot root:** Approved. Use a `--snapshot-root` flag with an environment fallback, defaulting to a directory under the user's own Leko data folder. Tests use temporary roots and never write to `/var/lib/leko`.

5. **Challenge timeout:** Approved. Five seconds by default, overridable per spec.

Test routing: snapshot-pin tests go into the existing `core/tests/native_verifier.rs` rather than a ninth file in `core/tests/`. AC-015 checks guardrails and an end-to-end add-on CLI call from Python; it does not run the suite from inside a discovered test.

Verification: `~/.cargo/bin/cargo test && /Users/calibro/.t3/scratch/phone-pilot/talk-venv/bin/python3 -m unittest discover -s tests -p 'test_*.py' -v`, passed to `/contract done --run`.
