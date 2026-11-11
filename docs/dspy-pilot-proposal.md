# DSPy pilot proposal

**Status:** Research proposal only. No DSPy dependency or runtime change has
been made.

## Goal

Test whether DSPy can improve Leko's local intent and field extraction using a
labeled evaluation set, instead of relying only on hand-edited prompt wording.
DSPy provides [typed task signatures](https://dspy.ai/) and [optimizers that
score candidate programs against examples and a metric](https://dspy.ai/learn/optimization/optimizers/).
It documents [OpenAI-compatible endpoint support](https://dspy.ai/getting-started/installation/),
but that does not guarantee compatibility with Leko's LFM3 and llama.cpp
tool-call behavior.

## What DSPy would and would not own

- **Pilot model call:** one DSPy `Predict` module proposes the action and
  fields supported by the current utterance. Keep the output contract small
  and typed.
- **Application code:** owns the active request checklist, field provenance,
  merge and correction rules, validation, the next clarification, preview,
  and confirmation. DSPy does not replace this state machine.
- **Existing safeguards:** keep CLI execution and the backend-owned
  confirmation gate outside the model and DSPy.
- **Out of scope:** no DSPy ReAct loop for live actions, no transcript RAG, no
  extra runtime model call, and no automatic execution from model output.

## Suggested experiment

1. Create a reviewed set of voice-like requests and multi-turn corrections,
   with expected action, field values, and next missing field.
2. Run the current LFM3 prompt as the baseline and save its outputs and timing.
3. Build a DSPy signature for a turn-local field patch and run `Predict` against
   the same local model endpoint. Keep app-owned dialogue state unchanged.
4. If the baseline comparison shows a real gap, try prompt/demo optimization
   against a rule-based metric. Keep training, validation, and final test
   examples separate. GEPA may require many candidate evaluations and can use
   a separate reflection model; treat that as offline compilation cost.
5. Save the winning program as an artifact and compare it on held-out cases
   before considering any runtime integration.

## Evaluation and adoption gate

Measure exact action and field match, volunteered-field capture, correction
handling, repeated questions for already supplied fields, safe abstention on
ambiguous input, model latency, and output-token count. The count of unconfirmed
mutations must remain zero. Adopt the pilot only if it improves held-out
accuracy and clarification behavior without exceeding the acceptable latency
budget.

Do not use an LLM judge for confirmation safety or exact field correctness.
Report results separately for ordinary requests, follow-ups, corrections,
ambiguous targets, schedule polarity, and noisy speech transcriptions.

## Risks and limits

- An optimizer only improves what the examples and metric reward; a weak or
  narrow dataset can produce a prompt that overfits.
- DSPy signatures help define an output contract but do not guarantee that a
  small model interpreted the user's meaning correctly.
- OpenAI-compatible endpoint support does not verify LFM3's structured output,
  reasoning, or tool-call template in this local server. Test the exact path.
- DSPy's repository is [MIT-licensed](https://github.com/stanfordnlp/dspy/blob/main/LICENSE).
  Review and pin a version before adopting it as a runtime dependency.
- Prompt optimization should remain an offline development step. A compiled
  prompt can be loaded at inference without running the optimizer per turn.

## Recommendation

Run a small offline `Predict` comparison first. Do not replace Leko's workflow
or add DSPy to the production voice path unless the held-out evaluation shows a
clear improvement. The higher-priority reliability gap remains deterministic
active request state, as described in
[voice-workflow-follow-ups.md](voice-workflow-follow-ups.md).
