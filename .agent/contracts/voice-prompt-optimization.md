# Contract: voice-prompt-optimization

Written before any code. After the user opens it, the contract is locked: criteria cannot be edited to match the code.
Every "<...>" part must be replaced with real text. `/contract open <feature>` refuses a contract that is not complete.

## Goal
Re-enable reasoning, expand context window, fix sampling params, and optimize the system prompt so the voice assistant handles corrections, asks for clarification when unsure, and avoids repetitive responses.

## Domains
Domains: none

## Acceptance criteria
- AC-001: llama-server starts with reasoning enabled (no --reasoning off flag) and ctx-size at least 8192.
  - evidence: tests/test_llm_config.py "reasoning enabled and ctx expanded"
  - breaker: server starts with reasoning off or ctx below 8192.
- AC-002: Sampling params include top_k=50 and repetition_penalty=1.1 in the request payload.
  - evidence: tests/test_llm_config.py "sampling params present"
  - breaker: request payload omits top_k or repetition_penalty.
- AC-003: System prompt includes correction-awareness instruction telling the model to acknowledge corrections before acting.
  - evidence: tests/test_llm_config.py "correction awareness in prompt"
  - breaker: system prompt has no correction or clarification guidance.
- AC-004: System prompt includes an uncertainty rule: ask a short clarifying question instead of guessing when intent is unclear.
  - evidence: tests/test_llm_config.py "uncertainty rule in prompt"
  - breaker: system prompt has no ask-when-unsure instruction.
- AC-005: History window reduced to 4 turns and memories capped at 2 to fit the small model's context budget.
  - evidence: tests/test_llm_config.py "history and memory caps"
  - breaker: history exceeds 4 turns or memories exceed 2 in the request.
- AC-006: preserve_thinking is enabled so reasoning from prior turns is retained in context.
  - evidence: tests/test_llm_config.py "preserve thinking enabled"
  - breaker: prior turn reasoning is dropped from context.

## Derived checklist
- D-1: Reasoning flag removed from server command -> criterion:AC-001
- D-2: ctx-size value verified >= 8192 -> criterion:AC-001
- D-3: top_k=50 in payload -> criterion:AC-002
- D-4: repetition_penalty=1.1 in payload -> criterion:AC-002
- D-5: Correction-awareness text present in system prompt -> criterion:AC-003
- D-6: Uncertainty/clarification rule present in system prompt -> criterion:AC-004
- D-7: History slice limited to last 4 turns -> criterion:AC-005
- D-8: Memories slice limited to 2 -> criterion:AC-005
- D-9: preserve_thinking passed in request or chat template -> criterion:AC-006

## Edge cases
- Empty user input -> out-of-scope: handled by existing VAD gate, not this feature.
- Model server already running with old config -> criterion:AC-001
- History shorter than 4 turns -> criterion:AC-005
- No memories available -> criterion:AC-005

## Out of scope
- Intent router / cascade classification (separate experiment).
- Distillation or training data collection.
- UI changes for streaming partial text.
- Changing the STT model.

## Decisions
- Remove --reasoning off to re-enable reasoning | model was trained with reasoning ON; fighting it reduces accuracy | REVERSIBLE
- ctx-size 8192 not 50000 | 1.7B model on CPU; 50K would be unusably slow; 8K is practical | REVERSIBLE
- History 4 turns, memories 2 | reduces context load for 1.7B model; keeps most relevant context | REVERSIBLE
- Add correction-awareness and uncertainty rules to existing prompt | addresses mishearing/correction issue without adding a separate prompt | REVERSIBLE