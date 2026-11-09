import test from 'node:test';
import assert from 'node:assert/strict';
import { runPython } from './python-runner.mjs';

const CONTINUITY_SCRIPT = `
import numpy as np
from voice.turn_detector import TurnDetector

detector = TurnDetector(speech_detector=False)
t = np.linspace(0, 1.0, 16000, endpoint=False)
speech = (np.sin(2 * np.pi * 400 * t) * 12000).astype(np.int16)
chunk_speech = speech[:2560]
detector.feed(chunk_speech)
assert detector.in_speech, "Detector should be in speech"

silence_chunk = np.zeros(2560, dtype=np.int16)
ended_1, _ = detector.feed(silence_chunk)
ended_2, _ = detector.feed(silence_chunk)
assert not ended_1 and not ended_2, "320ms pause must not prematurely end the turn"
assert detector.in_speech, "Detector must preserve in_speech across 320ms pause"
detector.feed(chunk_speech)
turn_ended = False
for _ in range(34):
    ended, audio = detector.feed(silence_chunk)
    if ended:
        turn_ended = True
        assert len(audio) > 0
        break

assert turn_ended, "Sustained quiet must conclude turn"
print("OK")
`;

const NOISE_FLOOR_SCRIPT = `
import numpy as np
from voice.turn_detector import TurnDetector

detector = TurnDetector()
low_noise = (np.random.randn(16000) * 60).astype(np.int16)
for i in range(0, len(low_noise), 2560):
    detector.feed_ambient(low_noise[i:i+2560])
thr_low = detector.speech_threshold
high_noise = (np.random.randn(16000) * 200).astype(np.int16)
for i in range(0, len(high_noise), 2560):
    detector.feed_ambient(high_noise[i:i+2560])
thr_high = detector.speech_threshold
assert thr_high > thr_low, f"Threshold should adapt upward ({thr_high} vs {thr_low})"
print("OK")
`;

async function assertPythonSuccess(script, label) {
  const { code, stdout, stderr } = await runPython(script);
  assert.equal(code, 0, `${label} failed: ${stderr}`);
  assert.ok(stdout.includes('OK'));
}

test('turn_detector_preserves_continuity_over_short_pauses', async () => {
  await assertPythonSuccess(CONTINUITY_SCRIPT, 'Turn detector test');
});

test('noise_floor_calibration_adapts_to_ambient_level', async () => {
  await assertPythonSuccess(NOISE_FLOOR_SCRIPT, 'Noise floor calibration test');
});
