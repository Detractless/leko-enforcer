# Microphone continuation window

Before this adjustment, the detector could submit after four quiet 160ms
frames (640ms) if turn-1-mini reported an endpoint above 0.65, or after eight
quiet frames (1.28s) regardless of model availability or confidence.

The detector requires five seconds of continuous classified quiet before
submitting audio for transcription. This is five seconds total after the last
detected speech, not five seconds added after the earlier endpoint. Capture
supplies 2560-sample frames at 16kHz (160ms), assembled from 1280-sample device
callbacks (80ms). The first frame at or beyond five seconds is frame 32, about
5.12 seconds. Acoustic or fallback endpoint detection may happen sooner, but
does not submit the turn early. These are sample-clock durations of classified
quiet frames; physical speech boundaries, VAD behavior, buffering and scheduling
can shift wall-clock latency. No timer expires without captured audio arriving.

The microphone stays active throughout this window. Any non-quiet frame clears
the pending endpoint and quiet counters, retaining all audio in the same turn.
The next pause must satisfy a fresh five-second window. Acoustic endpoint
evidence remains latched only within uninterrupted quiet. No transcription,
model work, or response begins during this window. The existing 60-second
buffered-turn hard cap (including pre-roll and pauses) remains an exception and
can submit while speech continues; this change does not extend that cap.

Once submitted, CaptureGate fences callbacks immediately and stops/closes the
actual input stream on its control thread. The worker waits for that closure
before transcription. Capture remains closed through transcription, thinking,
synthesis and playback, followed by the existing 0.7-second response cooldown.
There is no speech interruption or transcription-based incomplete-clause
reopening. Stale callbacks cannot contribute audio to later turns.

Mute, stop/cancel and shutdown reset pending endpoints along with buffered audio.
Typed/UI submissions retain their immediate response gate and discard a pending
audio turn. Memory and confirmation handling are unchanged.

Device-free tests in `tests/test_continuation_window.py` exercise joined speech,
exact frame boundaries, fallback latency, single submission, response gating,
typed takeover and pending-window cleanup. Existing capture/runtime regressions
cover stale callbacks, delayed device closure, cancellation, all response phases,
UI flows and confirmations. Physical microphone timing and live UI/audio were
not tested; the owner must stop/restart the current process to load this change.
