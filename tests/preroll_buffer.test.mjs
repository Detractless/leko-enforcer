import test from 'node:test';
import assert from 'node:assert/strict';
import { runPython } from './python-runner.mjs';

test('preroll_buffer_captures_leadin_audio_before_voice_onset', async () => {
  const script = `
import numpy as np
from voice.turn_detector import TurnDetector

detector = TurnDetector(preroll_frames=5, speech_detector=False)

# Feed 4 frames of soft lead-in audio below speech threshold
soft_chunk = (np.ones(2560, dtype=np.int16) * 150)
for _ in range(4):
    detector.feed(soft_chunk)

assert not detector.in_speech, "Should not be in speech yet"
assert len(detector.preroll) == 4, f"Pre-roll should hold 4 frames, got {len(detector.preroll)}"

# Now feed loud frame that crosses threshold
loud_chunk = (np.ones(2560, dtype=np.int16) * 8000)
detector.feed(loud_chunk)

assert detector.in_speech == True, "Should now be in speech"
assert len(detector.buf) == 5, f"Buffer must contain 4 pre-roll frames + 1 loud frame, got {len(detector.buf)}"
print("OK")
`;

  const { code, stdout, stderr } = await runPython(script);
  assert.equal(code, 0, `Preroll test failed: ${stderr}`);
  assert.ok(stdout.includes('OK'));
});

test('leadin_syllables_are_preserved_in_captured_turn', async () => {
  const script = `
import numpy as np
from voice.turn_detector import TurnDetector

detector = TurnDetector(preroll_frames=5, min_sil_frames=2, max_sil_frames=4, speech_detector=False, continuation_s=0)

# Feed 5 pre-roll frames
for _ in range(5):
    detector.feed(np.ones(2560, dtype=np.int16) * 150)

# Feed 2 speech frames
speech_chunk = (np.ones(2560, dtype=np.int16) * 6000)
detector.feed(speech_chunk)
detector.feed(speech_chunk)

# Feed quiet frames to end turn
silence = np.zeros(2560, dtype=np.int16)
turn_ended = False
for _ in range(5):
    ended, audio = detector.feed(silence)
    if ended:
        turn_ended = True
        # Expected frames: 5 pre-roll + 2 speech + quiet frames
        expected_min_samples = (5 + 2) * 2560
        assert len(audio) >= expected_min_samples, f"Captured audio should include pre-roll ({len(audio)} >= {expected_min_samples})"
        break

assert turn_ended == True, "Turn should conclude"
print("OK")
`;

  const { code, stdout, stderr } = await runPython(script);
  assert.equal(code, 0, `Leadin syllables test failed: ${stderr}`);
  assert.ok(stdout.includes('OK'));
});

test('llm_router_provides_current_time_context', async () => {
  const script = `
import datetime
from voice.llm_router import get_system_prompt

prompt = get_system_prompt()
now = datetime.datetime.now()
hour_12 = now.strftime("%I").lstrip("0")

assert "current time" in prompt.lower() or "current local time" in prompt.lower()
print("OK")
`;

  const { code, stdout, stderr } = await runPython(script);
  assert.equal(code, 0, `Time context test failed: ${stderr}`);
  assert.ok(stdout.includes('OK'));
});

test('preroll_buffer_bounded_and_cleared_on_turn_reset', async () => {
  const script = `
import numpy as np
from voice.turn_detector import TurnDetector

detector = TurnDetector(preroll_frames=5)

# Feed 30 quiet frames: pre-roll buffer must stay bounded at maxlen (5)
for _ in range(30):
    detector.feed(np.zeros(2560, dtype=np.int16))

assert len(detector.preroll) == 5, f"Preroll must be bounded at 5, got {len(detector.preroll)}"

detector.reset()
assert len(detector.preroll) == 0, "Preroll must be cleared on reset"
print("OK")
`;

  const { code, stdout, stderr } = await runPython(script);
  assert.equal(code, 0, `Bounded preroll test failed: ${stderr}`);
  assert.ok(stdout.includes('OK'));
});
