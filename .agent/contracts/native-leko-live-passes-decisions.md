# native-leko-live-passes decisions

Nothing is locked until the user types `/contract open native-leko-live-passes`. These are the choices only the user can make, each with one recommended answer.

1. **Pass state location:** Recommended: a separate versioned JSON state file beside the rule store, keyed by rule id. Alternative: embed pass state inside `rules.json`. A separate file keeps the rule definition and `updated_at` provably unchanged.

2. **Countdown wait:** Recommended: an injectable sleeper so tests are instant and live CLI runs really wait out `wait_seconds`. Alternative: always real time, which makes tests slow and flaky.

3. **Cap window:** Recommended: local calendar day in the configured zone, matching `LimitedPasses` semantics and `Period::Daily`. Alternative: a per-rule rolling window, which needs extra state and is not in the vocabulary.

4. **Delete ergonomics:** Recommended: `delete-rule` implies a delete edit by default, with Seals still fully enforced. The explicit `--direction delete` form stays accepted.

Verification: `~/.cargo/bin/cargo build --release && ~/.cargo/bin/cargo test && /Users/calibro/.t3/scratch/phone-pilot/talk-venv/bin/python3 -m unittest discover -s tests -p 'test_*.py' -v`, passed to `/contract done --run`.

Branch: `feat/m6-live-passes`, created off clean `main` at `ed687ac`. No live service, plist, binary or old checkout was touched for this draft.
