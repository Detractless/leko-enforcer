# Fable 5.2 review prompt: voice request state

Copy the prompt below into Fable 5.2 after opening the `voice-request-state-eval`
branch.

---

You are an independent reviewer of the Leko Enforcer repository:
https://github.com/Detractless/leko-enforcer/tree/voice-request-state-eval

Review that branch, not just `main`. If you cannot access the repository or branch,
say so clearly and ask for a clone, archive, or the relevant files. Do not pretend
you inspected code you could not access.

## Project goal

Leko is a local, privacy-focused voice assistant for focus sessions and blocking
apps, websites, or the whole computer. The voice workflow should understand
volunteered details and corrections, avoid asking again for information already
provided, validate consequential actions, and require confirmation before changes
are applied. A small local model proposes interpretations; deterministic code owns
request state, validation, preview, and confirmation.

The branch contains a request-state implementation and an offline comparison
between the existing LFM3 parser and a structured `request_update` extractor. The
owner wants an evidence-based diagnosis of why the structured version performed
poorly and what would make the workflow reliable. Treat these goals as requirements
to assess, not as proof that the current design is correct.

The branch also includes `.agents/skills/contract` and `.agents/skills/research-first`.
Check whether their instructions are usable from a fresh clone and identify any
host-installed tools they still require.

## Measured context

The synthetic held-out fixture has 5 cases and 8 turns. One local replay reported:

- Baseline: field accuracy 73.9%, correction success 100%, readiness accuracy
  100%, median model latency 8.84 seconds.
- Structured candidate: field accuracy 5.9%, correction success 0%, readiness
  accuracy 12.5%, redundant-question rate 87.5%, median model latency 29.98 seconds.

A traced rerun showed proposals with missing or mismatched field evidence, fields
that did not belong to the selected action, empty arrays, duplicate targets, and
noncanonical values. The exact proposals varied between runs. These are small,
synthetic results; do not present them as broad model benchmarks.

## Review tasks

1. Independently inspect the branch's implementation, prompts, tool schemas,
   validators, tests, fixtures, and workflow documents. Summarize what the app
   currently does and what the branch was intended to change.
2. Diagnose the observed failures. Separate directly observed facts, code-supported
   explanations, plausible hypotheses, and unknowns. Cite repository paths and
   function or test names for each load-bearing claim.
3. Assess whether the current split between the model and deterministic code is
   appropriate for a local 1.7B model. Compare the baseline parser, structured
   extraction, and the offline-only DSPy experiment where relevant. Identify
   simpler alternatives and their tradeoffs.
4. Recommend the smallest reliable changes, in priority order. Include how to
   represent action-specific fields, bind evidence to fields, handle corrections,
   avoid repeated clarifications, and evaluate quality. Do not recommend another
   model call unless evidence supports the extra latency.
5. Check portability, including any hard-coded machine paths, model dependencies,
   and what must change before a Colab run.
6. Can you test this setup on Google Colab? Answer yes or no first. If yes, state
   what repository access, runtime, hardware, packages, and owner actions are
   required; give reproducible commands and expected outputs. If no, explain the
   limitation and provide a local or manual alternative. Do not start a Colab job,
   upload private data, spend credits, or request credentials in this review.

## Boundaries and response

Do not edit files, push commits, open a pull request, or change repository
settings. Use only the synthetic fixtures and public source in this branch; do not
request voice recordings, local session logs, private model reasoning, tokens, or
other secrets. Give conclusions with concise evidence and rationale, not hidden
chain-of-thought.

Return:

- Short summary of the branch and its goal.
- Findings table: issue, evidence, likely cause, confidence, and consequence.
- Recommended fix sequence and concrete regression cases.
- Colab feasibility, prerequisites, and a safe test plan.
- Remaining uncertainties that would change your recommendation.
