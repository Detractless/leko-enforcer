# Voice workflow follow-ups

**Status:** Planning note only. No workflow code is changed by this note.

This note keeps the pending conversation-flow work alongside the speech-only
conversion rules. The speech proposals are listed in
[voice-speech-conversion-rules.md](voice-speech-conversion-rules.md).

## What the current conversation flow does

- The system prompt tells the model to ask whether an unclear block target is
  apps, websites, or the whole computer. It lists schedule meanings, but does
  not require one clarification question per turn.
- The model may return a free-form reply that summarizes known intent and asks
  several follow-up questions together. That recap is not a fixed application
  template.
- Conversation history is held in memory as up to six messages, with each
  message capped at 400 characters. The model request currently receives the
  latest four history messages plus the current request.
- There is no structured checklist for the active request. A separate backend
  slot stores one pending action for up to 120 seconds; it supports final yes/no
  confirmation, not progressive collection of request details.
- Explicit saved memory is separate from active request state. It is not an
  automatically maintained checklist of what the user has decided.

## Desired follow-up: structured request state

Keep validated fields for the active request in backend state. On each turn,
extract only new facts or corrections, merge them into that state, and retain
which user statement supplied each value. The backend should determine which
required field is still missing and ask one concise question for that field.
Fields the user already provided, including details given before they were
asked, should be marked complete and not asked again. If a value is ambiguous
or conflicting, ask about that value instead of guessing.

Candidate fields depend on the selected action. For a block, they may include:

- Target kind and target name(s), or the whole device.
- Device action when the whole device is targeted: lock, log out, or shut down.
- Schedule type: always, for a duration, open hours, or blocked hours.
- The matching schedule details: duration; allowed time range; or blocked time
  range and recurrence days.

The model can propose structured updates, but application code should validate
and own the active state and choose the next missing field. Do not rely only on
adding a prose checklist to the system prompt; recent conversation history can
be truncated, and free-form wording can repeat or contradict itself.

Keep internal tool-field names out of spoken explanations. A recent reply
exposed `open_hours` and `blocked_hours` in inline code; the speech layer
preserved those identifiers, so the voice read the underscore. Prefer human
labels such as “allowed hours” and “blocked hours” in the model's explanation,
with narrow TTS aliases as a fallback. Keep the actual structured tool fields
unchanged.

## Turn behavior to decide and test

1. Accept any clear fields volunteered by the user, even when they were not
   asked yet.
2. Ask one missing detail at a time. A short acknowledgement is optional; avoid
   repeating a recap as a second question in the same turn.
3. When required fields are complete, show the prepared action and use the
   existing backend confirmation gate before a change runs.
4. Treat corrections as updates to the matching field; clear or recompute
   dependent fields when the correction makes them invalid.
5. Test multi-field requests, volunteered details, corrections, ambiguous
   answers, intent changes, no repeated questions, and final confirmation.

## Speech-conversion status

The audio-only conversion targets are implemented and covered by focused text
tests. Their detailed rules and listening limits are recorded in
[voice-speech-conversion-rules.md](voice-speech-conversion-rules.md):

- Use the ellipsis pause for prose colons, preserving clock and technical
  colons. The schedule prompt comparison was listened to and approved.
- Speak a dash between recognized clock expressions as “to,” such as “nine
  a.m. to five p.m.”
- Resolve bare `m` from nearby duration or measurement evidence; preserve it
  when the context is weak or conflicting.

The structured request checklist and one-question-at-a-time workflow above
remain planning work. They were not changed as part of the speech conversion.
