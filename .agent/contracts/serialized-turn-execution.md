# Contract: serialized-turn-execution

Written before any code. After the user opens it, the contract is locked: criteria cannot be edited to match the code.
Every "<...>" part must be replaced with real text. `/contract open <feature>` refuses a contract that is not complete.

## Goal
Prevent concurrent request collision and frozen UI states by serializing turn processing, superseding stale turns upon speech cut-off, handling LLM HTTP errors gracefully, and guaranteeing UI reset in finally blocks.

## Domains
Domains: cli, ui

## Acceptance criteria
- AC-001: Speech turns are serialized via a single worker queue and turn sequence counter so concurrent requests never collide on the LLM backend.
  - evidence: tests/serialized_turns.test.mjs "serialized_turn_manager_prevents_concurrent_overlap"
  - breaker: Two rapid speech turns trigger concurrent overlapping threads hitting the router.
- AC-002: When the user cuts off the assistant and speaks a new turn, the older turn is superseded and dropped.
  - evidence: tests/serialized_turns.test.mjs "newer_turn_supersedes_stale_turn"
  - breaker: Assistant continues processing and speaking an older abandoned turn over user's new speech.
- AC-003: LLM router catches HTTP errors gracefully and returns fallback without crashing background thread.
  - evidence: tests/serialized_turns.test.mjs "llm_router_handles_http_errors_gracefully"
  - breaker: HTTP 500 error raises uncaught exception and kills the worker thread.
- AC-004: Speech lifecycle guarantees UI state reset back to idle/listening via try-finally blocks even on query failure.
  - evidence: tests/serialized_turns.test.mjs "ui_state_always_resets_on_query_failure"
  - breaker: Query processing failure leaves UI permanently stuck displaying 'Transcribing...'.

## Checklist coverage
- CLI-1 -> out-of-scope: Stdio voice loop
- CLI-2 -> criterion:AC-003
- CLI-3 -> criterion:AC-001
- UI-1 -> criterion:AC-004
- UI-2 -> criterion:AC-002
- UI-3 -> criterion:AC-004

## Edge cases
- LLM server responds with HTTP 500 or timeout -> AC-003
- User starts speaking mid-inference -> AC-002
- Multiple rapid voice bursts in noisy room -> AC-001

## Out of scope
- Cloud-hosted fallback LLM.
- Distributed queuing system (in-process serialized threading queue only).

## Decisions
- Serialized worker queue with sequence counter | Eliminates thread races and prevents LLM slot collisions | REVERSIBLE
- Try/finally state reset guarantee | Ensures the UI never freezes on transient network or generation errors | REVERSIBLE
