# Contract: voice-intent-router

Written before any code. After the user opens it, the contract is locked: criteria cannot be edited to match the code.
Every "<...>" part must be replaced with real text. `/contract open <feature>` refuses a contract that is not complete.

## Goal
Add an optional two-stage intent router: a fast classifier categorizes the utterance (tool_call, conversational, correction, status_check), then a specialized system prompt is used for the second pass. Behind a config flag so single-prompt remains the default.

## Domains
Domains: none

## Acceptance criteria
- AC-001: A classify_intent function returns one of: tool_call, conversational, correction, status_check.
  - evidence: tests/test_intent_router.py "classify returns valid category"
  - breaker: classifier returns an invalid or empty category.
- AC-002: The router selects a specialized system prompt based on the classified category.
  - evidence: tests/test_intent_router.py "router selects specialized prompt"
  - breaker: all categories use the same prompt.
- AC-003: Router is gated by a config flag (ROUTING_MODE); default is "single" (no routing).
  - evidence: tests/test_intent_router.py "routing mode flag controls behavior"
  - breaker: routing always active regardless of flag.
- AC-004: When routing is enabled, parse_user_intent uses the classifier + specialized prompt and returns the same output format as single mode.
  - evidence: tests/test_intent_router.py "routed parse returns same format"
  - breaker: routed parse returns a different structure or crashes.
- AC-005: Correction category injects correction-aware context into the prompt.
  - evidence: tests/test_intent_router.py "correction context injected"
  - breaker: correction utterances use the generic prompt.

## Derived checklist
- D-1: classify_intent function exists and returns valid enum -> criterion:AC-001
- D-2: Four specialized prompts defined (tool, conversational, correction, status) -> criterion:AC-002
- D-3: ROUTING_MODE env var or config controls activation -> criterion:AC-003
- D-4: parse_user_intent checks flag before routing -> criterion:AC-003
- D-5: Routed path returns dict with action/tool/arguments -> criterion:AC-004
- D-6: Correction prompt includes prior-turn context -> criterion:AC-005
- D-7: Single mode unchanged when flag is off -> criterion:AC-004
- D-8: Classifier uses short prompt (under 100 tokens) -> criterion:AC-001

## Edge cases
- Empty utterance -> out-of-scope: handled by VAD gate, not this feature.
- Classifier returns unknown category -> criterion:AC-001
- No history available for correction -> criterion:AC-005
- Server not running -> criterion:AC-004

## Out of scope
- Distillation or training data collection.
- UI changes.
- Changing the STT model.
- Measuring latency improvements (separate benchmark).

## Decisions
- Router behind config flag, default off | preserves existing behavior; experiment safely | REVERSIBLE
- Classifier uses same LFM3 model with short prompt | no extra model needed; latency tradeoff accepted | REVERSIBLE
- Four categories only | covers all tool types without over-engineering | REVERSIBLE