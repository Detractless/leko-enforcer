# Contract: rolling-preroll-buffer

Written before any code. After the user opens it, the contract is locked: criteria cannot be edited to match the code.
Every "<...>" part must be replaced with real text. `/contract open <feature>` refuses a contract that is not complete.

## Goal
Implement a rolling pre-roll audio buffer in TurnDetector to capture soft lead-in words before voice activity detection, and inject real-time clock context into the assistant prompt.

## Domains
Domains: cli

## Acceptance criteria
- AC-001: TurnDetector maintains a rolling circular buffer of pre-trigger audio (0.8s pre-roll) and prepends it to the turn upon voice onset.
  - evidence: tests/preroll_buffer.test.mjs "preroll_buffer_captures_leadin_audio_before_voice_onset"
  - breaker: Audio frames prior to speech threshold crossing are discarded.
- AC-002: Soft lead-in utterance syllables ('can you tell me') are preserved and included in the captured turn audio.
  - evidence: tests/preroll_buffer.test.mjs "leadin_syllables_are_preserved_in_captured_turn"
  - breaker: Captured audio only starts at the highest amplitude spike, cutting off opening words.
- AC-003: LLM router injects current local system time into prompt context so conversational time queries receive accurate answers.
  - evidence: tests/preroll_buffer.test.mjs "llm_router_provides_current_time_context"
  - breaker: Time query returns generic 'I do not have access to real-time information'.
- AC-004: Pre-roll cache clears upon turn conclusion and remains active during continuous ambient listening without memory leakage.
  - evidence: tests/preroll_buffer.test.mjs "preroll_buffer_bounded_and_cleared_on_turn_reset"
  - breaker: Pre-roll buffer grows unbounded over long listening sessions.

## Checklist coverage
- CLI-1 -> out-of-scope: In-process audio streaming
- CLI-2 -> criterion:AC-003
- CLI-3 -> criterion:AC-001

## Edge cases
- Extremely soft whisper start followed by loud exclamation -> AC-002
- Long silence between turns -> AC-004
- Ambient noise spike without speech -> AC-001

## Out of scope
- Storing permanent audio logs to disk.
- Third-party weather or web search APIs.

## Decisions
- 0.8s circular pre-roll deque | Exactly matches human phoneme lead-in duration (5 chunks = 800ms) with zero latency impact | REVERSIBLE
- Inject local clock into system prompt | Allows the assistant to answer time queries naturally on-device | REVERSIBLE
