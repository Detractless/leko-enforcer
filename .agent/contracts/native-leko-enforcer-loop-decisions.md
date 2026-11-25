# native-leko-enforcer-loop decisions

Resolved: the user approved all five recommended answers. Nothing is locked until the user types `/contract open native-leko-enforcer-loop`.

1. **Default loop mode:** Approved. Continuous by default, with `--once` as the opt-out.

2. **LaunchAgent `KeepAlive`:** Approved. Keep it true in generated plists; the loop has clean signal shutdown and is intended to run continuously.

3. **OS executor injection:** Approved. Use an injected trait with mock implementations in tests, so no test can reach a real macOS API.

4. **Website coordination:** Approved. Read browser heartbeats through the existing protected native enforcement contract and native messaging coordination, without editing protected paths.

5. **Whole Device `Restart`:** Approved. Report it as unavailable because the macOS adapter currently refuses it.

Reference source: all pre-rewrite behavior was read from this clone's git history at `a21f813`. The checkout at `/Users/calibro/Projects/leko-enforcer` and all live services were left untouched.

Test routing: loop tests go into existing `cli/tests/` files plus `cli/src/services/os_executor.rs`, so no directory grows past eight entries and no file approaches the 350-line cap.

Verification: `~/.cargo/bin/cargo build --release && ~/.cargo/bin/cargo test && /Users/calibro/.t3/scratch/phone-pilot/talk-venv/bin/python3 -m unittest discover -s tests -p 'test_*.py' -v`, passed to `/contract done --run`. The release build runs first so `target/release/leko` is not stale from M4.
