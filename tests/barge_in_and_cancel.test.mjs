import test from 'node:test';
import assert from 'node:assert/strict';
import { runPython } from './python-runner.mjs';

const STOP_ACTION_SCRIPT = `
import time
from voice.assistant import SerializedTurnWorker, process_query
from voice.ui_bridge import UIPipelineBridge

worker = SerializedTurnWorker()
bridge = UIPipelineBridge()

emitted_events = []
def mock_emit(ev):
    emitted_events.append(ev)
bridge.emit = mock_emit

# Submit slow query
spoken_replies = []
def mock_speak(txt, is_cancelled=None):
    if is_cancelled and is_cancelled():
        return
    spoken_replies.append(txt)

def run_turn(seq):
    # Simulate turn that checks is_cancelled
    time.sleep(0.05)
    if worker.is_superseded(seq):
        return
    process_query("What time is it?", "/tmp/settings.zip", bridge=bridge, is_cancelled=lambda: worker.is_superseded(seq), speak_fn=mock_speak)

seq = worker.submit(run_turn)
# Immediately cancel / stop
worker.cancel_current()
time.sleep(0.2)

# Ensure no draft or reply was spoken
assert len(spoken_replies) == 0, f"Expected 0 spoken replies after cancel, got {spoken_replies}"
speaking_events = [e for e in emitted_events if e.get("type") == "state" and e.get("state") == "speaking"]
assert len(speaking_events) == 0, f"Expected no speaking events after stop, got {speaking_events}"
print("OK")
`;

test('stop_action_aborts_inflight_generation_and_discards_reply', async () => {


  const { code, stdout, stderr } = await runPython(STOP_ACTION_SCRIPT);
  assert.equal(code, 0, `Stop action abort test failed: ${stderr}`);
  assert.ok(stdout.includes('OK'));
});

const CANCELLATION_TOKEN_SCRIPT = `
from voice.assistant import process_query, speak
from voice.ui_bridge import UIPipelineBridge

bridge = UIPipelineBridge()
emitted = []
bridge.emit = lambda ev: emitted.append(ev)

spoken = []
def mock_speak(txt, is_cancelled=None):
    if is_cancelled and is_cancelled():
        return
    spoken.append(txt)

# Test 1: pre-cancelled query
res = process_query("Block reddit", "/tmp/settings.zip", bridge=bridge, is_cancelled=lambda: True, speak_fn=mock_speak)
assert res == "" or res is None, f"Expected empty result on cancelled query, got {res}"
assert len(spoken) == 0, "Spoken list must be empty"
assert len(emitted) == 0, "No UI events should be emitted for cancelled query"

print("OK")
`;

test('cancellation_token_guards_process_query_and_speech', async () => {


  const { code, stdout, stderr } = await runPython(CANCELLATION_TOKEN_SCRIPT);
  assert.equal(code, 0, `Cancellation guard test failed: ${stderr}`);
  assert.ok(stdout.includes('OK'));
});

const PLAYBACK_DECAY_SCRIPT = `
import numpy as np
import time
from voice.turn_detector import TurnDetector
onsets = []
detector = TurnDetector(speech_detector=False, on_speech_onset=lambda: onsets.append(True))

# Simulate assistant playback active
detector.set_playback_active(True)

# Feed loud audio while assistant playback is active.
playback = (np.ones(2560, dtype=np.int16) * 12000)
ended, audio = detector.feed(playback)
assert detector.in_speech == False
assert len(audio) == 0 and ended == False
assert len(detector.preroll) == 0, "Playback audio must not enter pre-roll"

# Keep the acoustic decay interval active, then re-arm with direct speech.
detector.set_playback_active(False, post_mute_s=0.05)
detector.set_playback_active(False, post_mute_s=0.05)
human_voice = (np.ones(2560, dtype=np.int16) * 4000)
detector.feed(human_voice)
assert detector.in_speech == False, "Room echo decay must remain guarded"
time.sleep(0.06)
detector.feed(human_voice)
assert detector.in_speech == True, "Microphone should re-arm after playback decay"
assert len(onsets) == 1
print("OK")
`;

test('playback_audio_is_discarded_and_user_speech_rearms_after_decay', async () => {


  const { code, stdout, stderr } = await runPython(PLAYBACK_DECAY_SCRIPT);
  assert.equal(code, 0, `Barge-in detection test failed: ${stderr}`);
  assert.ok(stdout.includes('OK'));
});

const PLAYBACK_GUARD_SCRIPT = `
import numpy as np
from voice.turn_detector import TurnDetector

detector = TurnDetector(speech_threshold=0.010)
detector.set_playback_active(True)

for level in (600, 2000, 12000):
    audio = np.ones(2560, dtype=np.int16) * level
    ended, buffered = detector.feed(audio)
    assert not ended and len(buffered) == 0
    assert detector.in_speech == False
    assert len(detector.preroll) == 0

print("OK")
`;

test('playback_guard_ignores_loud_and_quiet_playback_equally', async () => {


  const { code, stdout, stderr } = await runPython(PLAYBACK_GUARD_SCRIPT);
  assert.equal(code, 0, `Threshold bleed vs voice test failed: ${stderr}`);
  assert.ok(stdout.includes('OK'));
});

const AEC_CLEAN_SPEECH_SCRIPT = `
import numpy as np
from voice.turn_detector import TurnDetector

onsets = []
detector = TurnDetector(speech_detector=False, speech_threshold=0.010, barge_in_threshold=0.035,
                        on_speech_onset=lambda: onsets.append(True))
detector.set_echo_cancellation_enabled(True)
detector.set_playback_active(True)

# This represents cleaned near-end speech: RMS is below the old bleed threshold.
speech = np.ones(2560, dtype=np.int16) * 600
detector.feed(speech)
assert detector.in_speech == False, "Microphone must be muted during playback to prevent self-interruption"
assert len(onsets) == 0

detector.set_playback_active(False, post_mute_s=0.0)
ended, out = detector.feed(speech)
assert detector.in_speech == True
assert len(onsets) == 1
print("OK")
`;

test('aec_clean_audio_allows_interruptions_at_normal_voice_threshold', async () => {


  const { code, stdout, stderr } = await runPython(AEC_CLEAN_SPEECH_SCRIPT);
  assert.equal(code, 0, `AEC barge-in test failed: ${stderr}`);
  assert.ok(stdout.includes('OK'));
});
