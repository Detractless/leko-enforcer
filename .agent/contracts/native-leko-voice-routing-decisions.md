# native-leko-voice-routing decisions

Resolved: the user approved all four recommended answers. Nothing is locked until the user types `/contract open native-leko-voice-routing`.

1. **Protected verbalizer:** Approved. Leave `voice/normalization/` byte-identical and emit exact Title Case canonical names from the routing layer before they reach the existing verbalizer.

2. **Planned-feature wording:** Approved. Reply with the fixed line "That feature is not available yet." and make zero CLI calls, optionally naming a live alternative in the same reply.

3. **CLI binary for tests:** Approved. Keep the existing `LEKO_BIN` constant for production and allow an environment override so tests point at a temporary store and injected timestamp.

4. **Files touched in voice/session:** Approved. Change `voice/routing/` and the CLI-call and status-formatting parts of `voice/core/assistant.py` and `voice/session/query_flow.py`, and leave the 349-line `voice/session/runtime.py` untouched or net-zero lines.

Test placement: new files go inside the existing `tests/python/routing/` and `tests/python/cli/` folders, so `tests/python/` stays at 6 immediate entries, within the cap of 8. `tests/python/voice/` already has 8 entries and receives no new files.

Verification: `~/.cargo/bin/cargo test && python3 -m unittest discover -s tests -p 'test_*.py' -v`, passed to `/contract done --run`. AC-015 checks only that no Python test asserts a legacy CLI command or flag and that the built binary is runnable, so no discovered test recurses into the suite.
