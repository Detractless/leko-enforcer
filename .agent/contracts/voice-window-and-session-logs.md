# Contract: voice-window-and-session-logs

Written before any code. This contract remains a draft until the user opens it.

## Goal
Keep the native window's current width while making it near-square and keeping the circle centered. Persist local voice-session records across restarts so transcripts, model reasoning, tool activity, results, and failures can be reviewed later.

## Domains
Domains: ui, data

## Acceptance criteria
- AC-001: The native window opens at 420 by 440 logical pixels, keeps the current 420-pixel width, and keeps the circle centered without clipping the circle or confirmation controls.
  - evidence: tests/ui_rendering.test.mjs "native_window_keeps_420x440_size_and_circle_centered"
  - breaker: The window stays tall, the circle shifts when the confirmation controls appear, or the resized content is clipped.
- AC-002: Each voice session writes a dated JSONL file under the Leko application-support folder, and later launches leave earlier session files intact.
  - evidence: tests/test_session_logs.py "test_session_log_persists_across_restart_without_overwrite"
  - breaker: Restarting discards the transcript or a new session replaces an older session file.
- AC-003: Each voice turn records the recognizer's text separately from the exact final text sent to the model, including empty, filtered, cancelled, or submitted outcomes.
  - evidence: tests/test_session_logs.py "test_records_stt_text_and_exact_submitted_text"
  - breaker: Turn filtering or editing silently loses the text that was heard or changes what was submitted without a trace.
- AC-004: Every local model call records its model and request settings, full message context, returned reasoning_content verbatim when present, user-facing content, and any tool-call draft.
  - evidence: tests/test_llm_config.py "test_router_preserves_reasoning_for_reply_and_tool_call"
  - breaker: The router discards the model's reasoning or request context before the session log can save it.
- AC-005: Tool-call records show the tool name and arguments, confirmation or cancellation, actual execution result or error, and related spoken reply and playback status.
  - evidence: tests/test_session_logs.py "test_records_tool_confirmation_result_and_playback"
  - breaker: A draft is recorded as a completed action or its later result and failure are missing.
- AC-006: Session files stay on the device with owner-only file permissions and the archive is not sent to a network service.
  - evidence: tests/test_session_logs.py "test_session_logs_are_private_local_files"
  - breaker: Session content is exposed to other local users or transmitted outside the device.
- AC-007: Archived sessions are not automatically added to live conversation history, explicit memory retrieval, or model prompts.
  - evidence: tests/test_session_logs.py "test_archived_sessions_are_not_loaded_as_memory_or_prompt_context"
  - breaker: Old transcripts or reasoning are silently reused as current facts or instructions.
- AC-008: A partial or failed event write leaves all earlier complete events readable and reports the incomplete record without corrupting the session archive.
  - evidence: tests/test_session_logs.py "test_partial_event_write_preserves_prior_records"
  - breaker: A crash or full disk makes earlier session records unreadable or silently drops the failure.
- AC-009: Missing or empty storage starts a new session cleanly; corrupt or unsupported records are reported and left untouched rather than overwritten.
  - evidence: tests/test_session_logs.py "test_missing_empty_and_corrupt_logs_are_handled_without_overwrite"
  - breaker: Startup silently replaces an existing archive or treats invalid data as a valid session.
- AC-010: Every record carries a schema version, and records from an earlier supported version remain readable after an application update.
  - evidence: tests/test_session_logs.py "test_versioned_session_records_remain_readable"
  - breaker: A format change makes saved history unreadable with no recovery path.

## Checklist coverage
- UI-1 -> out-of-scope: This feature adds no screen or interactive panel; it resizes the existing voice window.
- UI-2 -> criterion:AC-001
- UI-3 -> criterion:AC-001
- DATA-1 -> criterion:AC-008
- DATA-2 -> criterion:AC-009
- DATA-3 -> criterion:AC-010

## Edge cases
- The recognizer returns empty text, the turn guard filters it, or the user cancels before submission -> criterion:AC-003
- A model response has no reasoning_content field, or returns a conversational reply instead of a tool call -> criterion:AC-004
- The process stops or the disk fills during an event write -> criterion:AC-008
- An older archive is empty or corrupt -> criterion:AC-009
- An archive has an unsupported schema version -> criterion:AC-010
- Two sessions start close together and must not share or replace a filename -> criterion:AC-002

## Out of scope
- Saving raw microphone audio or every partial audio chunk by default; the decision page asks whether those should be included.
- A transcript browser, search screen, export/share flow, cloud backup, or analytics service.
- Feeding archived logs into RAG or live assistant memory.
- Changes to the speech model, LLM prompt, turn-taking behavior, or block execution workflow.

## Decisions
- Start the native window at 420 by 440 logical pixels, keeping the existing width | It is near-square while leaving room for the circle and controls | REVERSIBLE
- Use one local JSONL file per session in date folders | It is appendable, inspectable, and keeps sessions separate | REVERSIBLE
- Save model-emitted reasoning_content and the request context exactly as returned or sent | This gives the user evidence to diagnose model decisions | REVERSIBLE
- Keep archived logs separate from live memory and prompt retrieval | Old private content should not silently become current context or authorization | REVERSIBLE
- Do not store raw microphone audio by default | Text and capture metadata are useful while using less disk and retaining less biometric data | REVERSIBLE
- Keep session files until the user deletes them; do not auto-purge | This preserves logs across restarts and future debugging | REVERSIBLE
