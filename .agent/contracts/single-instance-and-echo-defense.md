# Contract: single-instance-and-echo-defense

Written before any code. After the user opens it, the contract is locked: criteria cannot be edited to match the code.
Every "<...>" part must be replaced with real text. `/contract open <feature>` refuses a contract that is not complete.

## Goal
Enforce a single running instance of the UI/assistant, prevent speaker echo and self-transcription loop with post-speech mute decay, suppress AUHAL abort errors, and immediately reflect active speech in the UI.

## Domains
Domains: cli, ui

## Acceptance criteria
- AC-001: Assistant startup terminates any previous instance and prevents concurrent duplicate UI windows.
  - evidence: tests/instance_echo.test.mjs "single_instance_prevents_duplicate_windows"
  - breaker: Running assistant twice opens two simultaneous UI windows.
- AC-002: Acoustic echo defense mutes microphone during audio playback and extends decay for 0.7s after playback to prevent self-hearing loop.
  - evidence: tests/instance_echo.test.mjs "echo_defense_mutes_during_and_after_playback"
  - breaker: Assistant transcribes its own synthesized voice output.
- AC-003: TurnDetector emits speech onset event immediately when voice activity begins, updating UI to listening state.
  - evidence: tests/instance_echo.test.mjs "turn_detector_emits_speech_onset_for_ui"
  - breaker: UI remains stuck in transcribing or ready state while user is actively speaking.
- AC-004: Stop action safely halts audio playback without raising unhandled AUHAL error exceptions.
  - evidence: tests/instance_echo.test.mjs "stop_playback_safely_halts_audio"
  - breaker: Halting playback throws uncaught AUHAL -50 exception.

## Checklist coverage
- CLI-1 -> out-of-scope: Process supervisor handles PID lifecycle
- CLI-2 -> criterion:AC-001
- CLI-3 -> criterion:AC-002
- UI-1 -> criterion:AC-003
- UI-2 -> criterion:AC-004
- UI-3 -> criterion:AC-003

## Edge cases
- Rapid double launch from terminal -> AC-001
- Speaker volume set to maximum on laptop speakers -> AC-002
- User interrupts assistant while thinking before speech begins -> AC-003

## Out of scope
- Hardware acoustic echo cancellation DSP driver installation.
- Multi-channel surround audio routing.

## Decisions
- PID-based single-instance lock | Ensures older background processes are terminated so only one window ever exists | REVERSIBLE
- Post-speech 0.7s quiet decay | Proven technique from phone-pilot to eliminate room reverb from speaker into mic | REVERSIBLE
