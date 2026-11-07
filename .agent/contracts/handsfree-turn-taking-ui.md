# Contract: handsfree-turn-taking-ui

Written before any code. After the user opens it, the contract is locked: criteria cannot be edited to match the code.
Every "<...>" part must be replaced with real text. `/contract open <feature>` refuses a contract that is not complete.

## Goal
Provide hands-free continuous speech turn-taking using phone-pilot's acoustic end-of-turn model and room-adaptive VAD, with an updated minimalist UI placing state text inside the circle and adding Mute and Stop controls.

## Domains
Domains: ui, cli

## Acceptance criteria
- AC-001: The minimalist UI renders state labels centered directly inside the ring core and adds discrete Mute and Stop control buttons.
  - evidence: tests/turn_taking_ui.test.mjs "ui_renders_centered_label_and_controls"
  - breaker: State text is rendered outside the ring or control buttons fail to render.
- AC-002: Tapping Mute toggles microphone capture and updates visual state, while Stop interrupts active speech playback or resets drafts.
  - evidence: tests/turn_taking_ui.test.mjs "controls_trigger_mute_and_stop_ipc_actions"
  - breaker: Mute does not alter mic active state or Stop fails to interrupt.
- AC-003: Turn-taking engine integrates acoustic end-of-turn scoring and adaptive VAD to automatically detect speech conclusion without manual tap-to-stop.
  - evidence: tests/turn_taking_ui.test.mjs "vad_and_turn_end_classifier_detect_speech_conclusion"
  - breaker: Speech pause always requires manual tap or prematurely cuts off mid-sentence speech.
- AC-004: Spoken query lifecycle automatically transitions from listening through thinking to Maeve speaking upon hands-free turn detection.
  - evidence: tests/turn_taking_ui.test.mjs "handsfree_pipeline_processes_query_on_turn_end"
  - breaker: Assistant stays in listening state after user finishes speaking.

## Checklist coverage
- UI-1 -> criterion:AC-001
- UI-2 -> criterion:AC-002
- UI-3 -> criterion:AC-001
- CLI-1 -> out-of-scope: UI controls run within WebKit/IPC
- CLI-2 -> criterion:AC-004
- CLI-3 -> criterion:AC-003

## Edge cases
- Ambient room noise during speech pause -> AC-003
- User presses Mute while assistant is speaking -> AC-002
- User speaks very short utterance or coughs -> AC-004

## Out of scope
- Multi-user diarization (identifying distinct speakers by voice).
- VoIP SIP integration.

## Decisions
- Center state text inside the ring | Creates an organic focal point and eliminates eye movement to separate pills | REVERSIBLE
- Port turn-1-mini ONNX classifier | Proven local 7MB acoustic model that accurately distinguishes mid-sentence pause from turn completion | REVERSIBLE
