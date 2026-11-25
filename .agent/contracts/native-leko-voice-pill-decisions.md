# native-leko-voice-pill decisions

Resolved: the user approved all four recommended answers. Nothing is locked until the user types `/contract open native-leko-voice-pill`.

1. **Retire the circle and wide UI:** Recommended: remove `ui/web/`, the `leko-ui` Rust crate and the circle and draft-card `.mjs` tests, keep the core voice IPC actions and states (`ptt_start`, `ptt_end`, `idle/listening/held/speaking/thinking/error`) for `runtime.py` and Python voice tests, and expose the pill as the single visual surface so `node --test tests/ui/*.mjs` passes cleanly. Alternative: keep the old UI dormant, which leaves two interfaces in the product.

2. **Pill implementation:** Recommended: a dependency-free Swift and AppKit binary built with the installed `swiftc` from `ui/pill/*.swift`, split into 2-4 small files to stay well under the 350-line cap, and emitted as `target/release/leko-pill`. Alternative: PyObjC, which adds a runtime dependency and startup latency.

3. **Event bridge:** Recommended: an atomic JSON event file beside `rules.json`, written by `leko enforce` on block events and read by the pill on a timer. Alternative: a Unix domain socket, which needs lifecycle and permission handling.

4. **Dictation trigger:** Recommended: map pill hold and click onto the existing voice UI actions without editing `voice/audio_io/` or `voice/normalization/`, and add no lines to the 349-line `voice/session/runtime.py`. Alternative: add a new runtime branch, which breaks the line cap.

Verification: `~/.cargo/bin/cargo build --release && swiftc ui/pill/*.swift -o target/release/leko-pill && ~/.cargo/bin/cargo test && /Users/calibro/.t3/scratch/phone-pilot/talk-venv/bin/python3 -m unittest discover -s tests -p 'test_*.py' -v && node --test tests/ui/*.mjs`, passed to `/contract done --run`.

Branch: `feat/m7-voice-pill`, created off clean `main` at `6e84c2f`. No live service, plist, binary or old checkout was touched for this draft.
