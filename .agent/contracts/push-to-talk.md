# Contract: push-to-talk

Written before any code. After the user opens it, the contract is locked: criteria cannot be edited to match the code.
Every "<...>" part must be replaced with real text. `/contract open <feature>` refuses a contract that is not complete.

## Goal
Let the user speak by holding the central circle or Space, then submit one combined utterance after a two-second release grace. Replace ambient hands-free turn detection with deliberate push-to-talk and keep the interaction usable by mouse, touch, and keyboard, with no visible UI text.

## Domains
Domains: ui, cli, web

## Acceptance criteria
- AC-001: A mouse or touch hold begun on the central circle starts one recording, and releasing it ends that hold even if the pointer leaves the circle or the window loses pointer focus.
  - evidence: tests/push_to_talk.test.mjs "circle_hold_starts_capture_and_release_outside_ends_hold"
  - breaker: Releasing outside the circle leaves capture active or loses the spoken segment.
- AC-002: Holding Space starts recording and releasing Space ends that hold; key repeat and editable fields never start duplicate or unwanted recordings.
  - evidence: tests/push_to_talk.test.mjs "space_hold_matches_pointer_without_key_repeat_or_editable_field_capture"
  - breaker: Space auto-repeat starts extra turns or typing in an editable field triggers recording.
- AC-003: After release, the app accepts a renewed hold for two seconds into the same audio turn, resets the grace period on each release, and submits exactly once when the full grace expires.
  - evidence: tests/test_push_to_talk.py "release_grace_merges_rehold_and_submits_once_after_two_seconds"
  - breaker: A resumed phrase becomes a separate turn, or the first release submits before the grace period ends.
- AC-004: Idle microphone audio cannot start transcription or a model request; only a held interaction and its release grace contribute audio to a turn, with no acoustic VAD endpoint or four-second continuation timer ending it.
  - evidence: tests/test_push_to_talk.py "idle_audio_is_discarded_and_manual_release_controls_turn_end"
  - breaker: Background noise triggers a query or the acoustic endpoint submits before the user finishes the hold interaction.
- AC-005: A new hold during Maeve's playback or an in-flight model response cancels that response and starts a new recording, and no stale output from the cancelled turn plays afterward.
  - evidence: tests/test_push_to_talk.py "hold_during_response_cancels_and_starts_new_capture"
  - breaker: A press during generation or playback fails to start capture, or the cancelled response later plays.
- AC-006: The UI contains no visible text, including labels, status or error messages, and draft details; visual changes convey the workflow, the bottom Mute and Stop controls are removed, and hidden screen-reader names and announcements provide accessible equivalents at small and large window sizes.
  - evidence: tests/push_to_talk.test.mjs "all_ui_states_are_textless_and_screen_reader_accessible"
  - breaker: A visible label, status, error, or control name remains, or assistive technology cannot identify the current state and controls.
- AC-007: Empty audio, duplicate down/up events, and microphone permission or device failures return the UI to a usable state without sending an empty or duplicate model request.
  - evidence: tests/test_push_to_talk.py "empty_audio_duplicate_events_and_capture_failure_are_safe"
  - breaker: A no-speech hold wedges the UI or sends multiple requests.
- AC-008: Before confirming a pending action, its target and schedule are spoken and available to screen readers without visible text, and icon-only confirm and cancel controls retain accessible names.
  - evidence: tests/push_to_talk.test.mjs "draft_details_and_icon_controls_remain_accessible_without_visible_text"
  - breaker: Users cannot verify the pending action or identify its confirm and cancel controls without visible text.

## Checklist coverage
- UI-1 -> criterion:AC-007
- UI-2 -> criterion:AC-002
- UI-3 -> criterion:AC-006
- CLI-1 -> out-of-scope: This feature adds no CLI command or argument parser.
- CLI-2 -> out-of-scope: This feature adds no command-line result or error output.
- CLI-3 -> criterion:AC-007
- WEB-1 -> out-of-scope: The feature uses the existing local UI bridge and adds no network request.
- WEB-2 -> criterion:AC-001
- WEB-3 -> criterion:AC-007

## Edge cases
- A press and release captures no speech -> AC-007
- The pointer is released outside the circle or loses focus -> AC-001
- Space repeats while held or an editable field has focus -> AC-002
- The user resumes speaking just before the two-second grace expires -> AC-003
- Microphone permission is denied after the UI opens -> AC-007
- The user starts a new hold during assistant output -> AC-005

## Out of scope
- Adding or replacing speech-recognition, voice-activity, or language models.
- Changing block, focus-session, draft-confirmation, or authorization rules.
- Changing Maeve's spoken responses or removing voice confirmation of pending actions.
- Persisting raw audio or transcripts to disk.
- Adding an automatic idle check-in prompt; source inspection found no such prompt. The existing four-second value is an acoustic continuation window after detected speech, not an idle timer.

## Decisions
- Use physical release plus a two-second grace as the turn endpoint | Lets the user resume one utterance while avoiding an acoustic end detector | REVERSIBLE
- Support mouse/touch hold on the circle and Space while the app is focused | Makes the same push-to-talk action available across mouse, touch, and keyboard | REVERSIBLE
- Let a new hold interrupt model generation or Maeve playback and start recording | Preserves immediate, buttonless interruption | REVERSIBLE
- Remove the bottom Mute and Stop controls; use Escape to cancel unsubmitted audio | Keeps the primary interface simple while retaining a keyboard cancel path | REVERSIBLE
- Remove all visible UI text and use visual state changes plus icon-only controls | Keeps the interface text-free while screen-reader labels and spoken action details preserve access | REVERSIBLE
- Scope Space to the focused app window and ignore it in editable fields | Avoids stealing normal text entry | REVERSIBLE
- Keep the existing confirmation flow for actions after transcription | Push-to-talk changes capture, not action approval | REVERSIBLE
