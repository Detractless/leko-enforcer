# Contract: zero-model-turn-taking

Written before any code. After the user opens it, the contract is locked: criteria cannot be edited to match the code.
Every "<...>" part must be replaced with real text. `/contract open <feature>` refuses a contract that is not complete.

## Goal
Prevent mid-sentence cut-offs without adding extra models using relaxed pause timing (650ms minimum, 1.2s cap), dynamic noise floor calibration, dangling-word grammatical guards, and audio turn merging.

## Domains
Domains: cli

## Acceptance criteria
- AC-001: TurnDetector maintains turn continuity during inter-clause pauses under 600ms and requires sustained quiet before evaluating end-of-turn.
  - evidence: tests/zero_model_turn.test.mjs "turn_detector_preserves_continuity_over_short_pauses"
  - breaker: Pause of 350ms triggers turn completion.
- AC-002: Dynamic noise floor calibration adapts the speech threshold to background ambient level, keeping soft speech active.
  - evidence: tests/zero_model_turn.test.mjs "noise_floor_calibration_adapts_to_ambient_level"
  - breaker: Noise floor remains static regardless of ambient volume changes.
- AC-003: Dangling word guard detects grammatically incomplete trailing tokens and grants a grace period for the speaker to continue.
  - evidence: tests/zero_model_turn.test.mjs "dangling_guard_identifies_incomplete_clauses"
  - breaker: Incomplete sentence ending on 'need' or 'think' immediately triggers command execution.
- AC-004: Turn manager merges subsequent speech arriving within the grace period into a unified query.
  - evidence: tests/zero_model_turn.test.mjs "turn_manager_merges_audio_within_grace_period"
  - breaker: Subsequent speech after a brief pause is dropped or causes duplicate processing.

## Checklist coverage
- CLI-1 -> out-of-scope: Turn detector is an in-process audio engine
- CLI-2 -> criterion:AC-004
- CLI-3 -> criterion:AC-001

## Edge cases
- Speaker coughs or clears throat before speaking -> AC-002
- Speaker pauses 1.5 seconds intentionally -> AC-001
- Sentence ends on a complete word but ambient noise is high -> AC-003

## Out of scope
- Adding external streaming neural networks (Parakeet-EOU, Whisper-streaming).
- Audio cloud upload.

## Decisions
- Zero extra models | Keeps app lightweight, low-power, and battery-friendly for mobile/Android | REVERSIBLE
- Dangling word lexicon | Catches natural hesitations ("need", "think", "and", "so") without heavy NLP parsing | REVERSIBLE
