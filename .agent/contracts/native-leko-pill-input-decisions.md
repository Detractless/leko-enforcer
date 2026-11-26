# native-leko-pill-input decisions

Nothing is locked until the user types `/contract open native-leko-pill-input`. These are the choices only the user can make, each with one recommended answer.

1. **Native cursor source:** Recommended: `NSEvent.mouseLocation` through AppKit behind a provider trait. Alternative: CoreGraphics `CGEventGetLocation`. Both see the cursor while another app has focus; AppKit keeps the dependency small and the trait keeps tests deterministic.

2. **Polling cadence:** Recommended: adaptive 16 ms near the pill and 90 ms away, matching token-monitor's proven pattern. Alternative: fixed 16 ms, which is simpler but wastes CPU when the pointer is far away.

3. **Click-through:** Recommended: start fully click-through and flip to mouse-accepting only while the cursor is inside the pill bounds. Alternative: always accept mouse events, which reproduces the current two-click problem because the first click is spent activating the window.

4. **First mouse:** Recommended: set AppKit's first-mouse acceptance so the first mouse-down is delivered without activating. Alternative: leave it unset and require two clicks.

5. **Hover feedback:** Recommended: inject `hover_on` and `hover_off` into the existing WebView state machine from the native poll. Alternative: rely on CSS `:hover`, which cannot fire while WebKit receives no mouse events.

Only the architecture was borrowed from the MIT-licensed token-monitor repo; no source code was copied.

Verification: `~/.cargo/bin/cargo build --release && ~/.cargo/bin/cargo test && /Users/calibro/.t3/scratch/phone-pilot/talk-venv/bin/python3 -m unittest discover -s tests -p 'test_*.py' -v && node --test tests/ui/*.mjs`, passed to `/contract done --run`.

Branch: `feat/pill-hover-first-click`, created off clean `main`. No live service, plist, binary or old checkout was touched for this draft.
