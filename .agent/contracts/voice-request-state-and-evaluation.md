# Contract: voice-request-state-and-evaluation

Written before any code. After the user opens it, the contract is locked: criteria cannot be edited to match the code.
Owner choices are listed on the [decision page](voice-request-state-and-evaluation-decisions.md).

## Goal

Make voice requests track the user's stated details across turns, apply corrections without asking for already supplied information again, and clarify one missing or ambiguous detail at a time. Prove the workflow with an offline replay evaluation and compare the current extractor with an optional DSPy extraction experiment before considering any production model change.

## Domains
Domains: none

## Acceptance criteria
- AC-001: The active request stores validated, typed fields by action and records the user turn or deterministic derivation that supplied each field.
  - evidence: tests/test_request_state.py "test_stores_typed_fields_with_source_turn"
  - breaker: a later turn loses a previously supplied field or cannot explain where a value came from.
- AC-002: New details and explicit corrections merge into the active request, and a correction clears or recomputes dependent fields.
  - evidence: tests/test_request_state.py "test_correction_updates_field_and_invalidates_dependent_values"
  - breaker: a correction leaves a stale schedule, target, or action value in the draft.
- AC-003: The backend asks one focused question for the highest-priority missing or ambiguous field and does not ask again for a clearly supplied field.
  - evidence: tests/test_query_flow.py "test_asks_one_missing_field_and_skips_volunteered_fields"
  - breaker: the assistant repeats a question, asks several questions in one turn, or asks for information the user already gave.
- AC-004: All current action types reject incomplete, conflicting, or invalid fields before a preview is staged.
  - evidence: tests/test_intent_validation.py "test_rejects_incomplete_or_conflicting_fields_for_every_action"
  - breaker: malformed values or an unresolved action reach the CLI preparation step.
- AC-005: A mutation still requires the backend-owned confirmation for its exact preview; changing or cancelling the active request makes an older confirmation unusable.
  - evidence: tests/test_runtime_confirmation.py "test_changed_request_cannot_use_stale_confirmation"
  - breaker: a stale or bare confirmation executes an outdated or unconfirmed mutation.
- AC-006: An offline replay runner compares the current baseline and structured-extraction candidate on the same held-out conversations and reports field accuracy, redundant-question rate, correction success, unsafe mutation count, and latency.
  - evidence: tests/test_voice_request_eval.py "test_reports_held_out_workflow_metrics"
  - breaker: prompt changes are judged from anecdotes or training examples instead of held-out results.
- AC-007: The DSPy candidate runs only in the offline evaluation path, and production voice routing remains on the current extractor.
  - evidence: tests/test_voice_request_eval.py "test_dspy_experiment_cannot_change_production_routing"
  - breaker: adding the experiment silently adds a runtime dependency, extra live model calls, or model-controlled action execution.

## Checklist coverage
Domains are none, so no domain checklist applies. Derived checks below cover state ownership, safety, evaluation, and scope.

## Derived checklist
- D-1: Active request data is separate from persistent saved facts and transcript logs -> criterion:AC-001
- D-2: Every accepted field has source-turn or deterministic-derivation provenance -> criterion:AC-001
- D-3: Corrections invalidate dependent values and replace the prior value -> criterion:AC-002
- D-4: Volunteered fields are merged before choosing a clarification -> criterion:AC-003
- D-5: The app, not the model, chooses one missing or ambiguous field to ask about -> criterion:AC-003
- D-6: All current action types are checked before CLI preview -> criterion:AC-004
- D-7: Confirmation remains bound to the exact staged action and expires or clears safely -> criterion:AC-005
- D-8: Replay results use held-out data and report safety separately from quality -> criterion:AC-006
- D-9: DSPy stays outside the live runtime until an explicit adoption decision -> criterion:AC-007

## Edge cases
- Empty or fragmented input such as "block" -> criterion:AC-003
- The user volunteers target, schedule, and days in the first utterance -> AC-003
- The user changes a blocked-hours request to allowed hours after some fields were collected -> criterion:AC-002
- A correction arrives after a preview was staged -> AC-005
- An overnight time range conflicts with the selected recurrence days -> AC-004
- A model proposal contains multiple actions or fields unsupported by the current utterance -> criterion:AC-004
- DSPy is not installed or its local endpoint adapter fails; baseline evaluation remains runnable -> criterion:AC-006

## Out of scope
- Replacing the speech recognizer, speech detector, or push-to-talk interaction.
- Adding vector RAG or feeding transcript logs into model context.
- Making DSPy or a second model call part of the live production path.
- Changing the UI, CLI action semantics, or the final confirmation requirement.
- Automatically executing a model proposal or inferring unspoken schedule values.

## Decisions
- Backend owns active request slots and clarification policy | prevents prompt history from acting as the only source of state | REVERSIBLE
- Keep active request state in memory and clear it on cancel, completion, or process restart | avoids reviving stale consequential drafts | REVERSIBLE
- Keep one model extraction call per user turn; code selects whether to ask, preview, or stop | preserves voice latency and makes workflow rules testable | REVERSIBLE
- Compare DSPy offline and keep it optional until held-out evidence supports adoption | separates prompt optimization from production safety | REVERSIBLE
- Keep persistent facts, active request state, and diagnostic logs separate | prevents old transcripts or saved facts from authorizing actions | REVERSIBLE
