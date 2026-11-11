# Voice request state and evaluation decisions

The contract is a draft. These are the owner choices to settle before opening it.

1. **Evaluation examples:** May the replay set use manually selected, redacted local utterances, or should it use synthetic examples only? **Recommended:** synthetic examples plus a small manually selected and redacted set; do not automatically import session logs or private reasoning.
2. **Request-state lifetime:** Should unfinished slot state survive an app restart? **Recommended:** no; clear unfinished requests and previews on restart, while keeping diagnostic logs separate.
3. **DSPy adoption:** Should a successful offline comparison add DSPy to the live runtime automatically? **Recommended:** no; keep the current runtime until you review held-out results and explicitly approve an integration.

## Defaults if accepted

- Use only reviewed evaluation examples; do not use private reasoning as a target or training signal.
- Keep active requests in process memory and clear them on cancel, completion, or restart.
- Treat DSPy as an optional offline candidate; keep production routing unchanged during evaluation.
