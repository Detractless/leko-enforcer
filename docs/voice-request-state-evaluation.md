# Voice request state and evaluation

Leko keeps one active request in memory while collecting details. Each accepted
field records the user turn and a short evidence quote, or a deterministic source
when the app derives it. The state is separate from saved facts and session logs;
it clears on cancel, successful completion, or process restart.

The local model proposes only fields from the latest turn. Python merges changes,
clears fields that no longer apply, validates the complete action, and asks one
question for the highest-priority missing or rejected field. A complete mutation
still goes through the existing CLI preview and backend-owned confirmation.
The request update uses one LFM3 call; DSPy is not imported by the live assistant.

## Offline replay

The reviewed synthetic corpus is `tests/fixtures/voice_request_cases.jsonl`. It is
marked `held_out`; session logs and private reasoning are not imported. The runner
does not invoke the CLI or write settings:

```sh
python3 tests/bench_llm.py --candidate structured
```

To compare an initial, unoptimized DSPy `Predict` candidate against the same local
LFM3 baseline, install DSPy in the voice environment and run:

```sh
python3 tests/bench_llm.py --candidate dspy
```

If DSPy is unavailable, the runner still reports the baseline and records the
candidate setup error. Metrics include field accuracy, redundant-question rate,
correction success, readiness accuracy, unsafe action proposals, unconfirmed
mutation count, model latency percentiles, and per-turn details. Because replay has
no executor, unconfirmed mutation count remains zero by construction; unsafe
proposals are reported separately.
