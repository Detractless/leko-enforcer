# Contract: push-to-talk-flow

Written before any code. After the user opens it, the contract is locked: criteria cannot be edited to match the code.
Every template placeholder must be replaced with real text. The open command refuses an incomplete contract.

## Goal
Let the user speak by holding the central circle or Space, then submit one combined utterance after a two-second release grace. Replace ambient hands-free turn detection in the UI with deliberate push-to-talk, while keeping the screen free of visible text and accessible to assistive technology.

## Domains
Domains: ui, cli, web

## Acceptance criteria
- AC-001: A mouse or touch hold started on the central circle begins one recording, and release outside the circle or loss of pointer focus ends that hold.
  - evidence: tests/push-to-talk-flow.test.mjs "circle_pointer_hold_captures_until_release_outside"
  - breaker: Releasing outside the circle leaves capture active or discards the held audio.
- AC-002: Holding Space starts recording and releasing Space ends it; key repeat and editable or actionable controls do not start duplicate or unwanted recordings.
  - evidence: tests/push-to-talk-flow.test.mjs "space_hold_matches_pointer_without_repeat_or_field_capture"
  - breaker: Space repeat starts extra turns or normal text and button input triggers recording.
- AC-003: After release, a renewed hold within two seconds joins the same audio turn, restarts the grace period, and the turn submits exactly once when grace expires.
  - evidence: tests/test_push_to_talk_flow.py "release_grace_merges_rehold_and_submits_once_after_two_seconds"
  - breaker: Resumed speech is split into another turn or the first release submits too early.
- AC-004: Idle microphone audio is discarded before echo processing and cannot start transcription or a model request; only a hold and its release grace contribute audio to a turn.
  - evidence: tests/test_push_to_talk_flow.py "idle_audio_is_discarded_until_hold_or_release_grace"
  - breaker: Room noise reaches transcription or the language model without a push-to-talk interaction.
- AC-005: Holding during model generation or Maeve playback cancels the old response and starts capture, and no stale output from the cancelled turn plays afterward.
  - evidence: tests/test_push_to_talk_flow.py "hold_interrupts_response_and_prevents_stale_playback"
  - breaker: The hold fails to interrupt output or the cancelled response plays after the new turn begins.
- AC-006: The UI has no visible text, including status, errors, controls, and draft details; visual changes show state, while screen readers receive state announcements and accessible names at small and large window sizes.
  - evidence: tests/push-to-talk-flow.test.mjs "ui_has_no_visible_text_and_announces_each_state"
  - breaker: Visible text remains, a state is indistinguishable, or assistive technology cannot identify the current state and controls.
- AC-007: Empty audio, duplicate start or end events, and microphone permission or device failures leave the UI usable without sending an empty or duplicate model request.
  - evidence: tests/test_push_to_talk_flow.py "empty_audio_duplicate_events_and_capture_failure_are_safe"
  - breaker: A no-speech hold wedges the interface or sends a duplicate request.
- AC-008: Before a pending action is confirmed, Maeve speaks its target and schedule, screen readers can access the same details, and icon-only confirm and cancel controls remain accessible.
  - evidence: tests/push-to-talk-flow.test.mjs "pending_action_details_and_icon_controls_remain_accessible"
  - breaker: Users cannot verify an action or identify its confirmation controls without visible text.

## Checklist coverage
- UI-1 -> criterion:AC-007
- UI-2 -> criterion:AC-002
- UI-3 -> criterion:AC-006
- CLI-1 -> out-of-scope: This feature adds no CLI command or argument parser.
- CLI-2 -> out-of-scope: This feature adds no command-line output or error handling.
- CLI-3 -> criterion:AC-007
- WEB-1 -> out-of-scope: The feature uses the existing local UI bridge and adds no network request.
- WEB-2 -> criterion:AC-001
- WEB-3 -> criterion:AC-007

## Edge cases
- A hold captures no speech -> AC-007
- Pointer release occurs outside the circle or the app loses focus -> AC-001
- Space repeats or a button has keyboard focus -> AC-002
- The user re-holds just before the two-second grace expires -> AC-003
- Microphone permission is denied after the interface opens -> AC-007
- The user interrupts Maeve while an action draft is pending -> criterion:AC-005

## Out of scope
- Adding or replacing speech-recognition, voice-activity, or language models.
- Changing block scheduling, focus-session, confirmation, or authorization rules.
- Persisting raw audio or transcripts to disk.
- Adding an automatic idle check-in prompt; source inspection found none. The existing four-second setting is an acoustic continuation window after detected speech, not an idle timer.
- Changing Maeve's spoken responses beyond stating pending action details before confirmation.

## Decisions
- Use physical release plus a two-second grace as the turn endpoint | Lets the user resume one utterance without an acoustic end detector | REVERSIBLE
- Support both mouse or touch circle hold and Space while the app is focused | Gives the same interaction across touch, pointer, and keyboard | REVERSIBLE
- Let a new hold cancel model generation or Maeve playback and start capture | Preserves immediate, buttonless interruption | REVERSIBLE
- Remove bottom Mute and Stop controls; use Escape to cancel audio before submission | Keeps the main screen simple while retaining a cancel path | REVERSIBLE
- Remove all visible UI text; keep hidden screen-reader labels, announcements, and accessible pending-action details | Preserves access without visible wording | REVERSIBLE
- Keep existing action-confirmation safeguards | Push-to-talk changes turn capture, not action approval | REVERSIBLE
