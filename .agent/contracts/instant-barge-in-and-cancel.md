# Contract: instant-barge-in-and-cancel

Written before any code. After the user opens it, the contract is locked: criteria cannot be edited to match the code.
Every "<...>" part must be replaced with real text. `/contract open <feature>` refuses a contract that is not complete.

## Goal
Implement end-to-end turn cancellation on Stop action and instant acoustic barge-in during AI generation and speech playback.

## Domains
Domains: cli

## Acceptance criteria
- AC-001: Clicking Stop halts playback, cancels in-flight LLM/TTS generation, and prevents previous turn response from reaching user or UI.
  - evidence: tests/barge_in_and_cancel.test.mjs "stop_action_aborts_inflight_generation_and_discards_reply"
  - breaker: In-flight LLM reply continues to play or display in the UI after Stop is triggered.
- AC-002: Process_query and speak check cancellation tokens before and after LLM inference and during speech synthesis, aborting without output when cancelled.
  - evidence: tests/barge_in_and_cancel.test.mjs "cancellation_token_guards_process_query_and_speech"
  - breaker: Cancelled queries still generate speech synthesis or update UI state.
- AC-003: Real-time acoustic barge-in detects user speech during playback, instantly interrupting active audio without requiring user to wait for completion.
  - evidence: tests/barge_in_and_cancel.test.mjs "barge_in_detects_speech_during_playback_and_halts_audio"
  - breaker: Microphone input is muted during playback, blocking user interruptions.
- AC-004: Barge-in threshold ignores assistant speaker bleed while triggering on direct user voice interruption.
  - evidence: tests/barge_in_and_cancel.test.mjs "barge_in_threshold_distinguishes_bleed_from_direct_voice"
  - breaker: Low-level speaker feedback continuously triggers false interruptions.

## Checklist coverage
- CLI-1 -> out-of-scope: Streaming audio playback
- CLI-2 -> criterion:AC-002
- CLI-3 -> criterion:AC-001

## Edge cases
- Stop clicked during slow model generation -> AC-001
- User speaks immediately after AI begins speaking sentence -> AC-003
- Ambient background noise while AI is speaking -> AC-004

## Out of scope
- Multi-microphone spatial beamforming hardware drivers.
- WebRTC AEC kernel extensions.

## Decisions
- Dual-mode VAD threshold | Uses ambient threshold for idle listening and elevated threshold (e.g. 0.035) during playback to enable interruption without self-echo | REVERSIBLE
- Token-based cooperative cancellation | In-flight turns exit early without throwing unhandled exceptions | REVERSIBLE
