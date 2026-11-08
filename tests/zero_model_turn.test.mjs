import test from 'node:test';
import assert from 'node:assert/strict';
import { existsSync } from 'node:fs';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawn } from 'node:child_process';

const __dirname = dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = resolve(__dirname, '..');
const TALK_PYTHON = '/Users/calibro/.t3/scratch/phone-pilot/talk-venv/bin/python3';
const PYTHON_BIN = existsSync(TALK_PYTHON) ? TALK_PYTHON : 'python3';

test('turn_detector_preserves_continuity_over_short_pauses', async () => {
  const script = `
import numpy as np
from voice.turn_detector import TurnDetector

detector = TurnDetector(speech_detector=False)

# 1 second of speech (400 Hz tone)
t = np.linspace(0, 1.0, 16000, endpoint=False)
speech = (np.sin(2 * np.pi * 400 * t) * 12000).astype(np.int16)

# Feed speech chunk
chunk_speech = speech[:2560]
detector.feed(chunk_speech)
assert detector.in_speech, "Detector should be in speech"

# Feed short pause of 2 chunks = 320 ms (must NOT cut off)
silence_chunk = np.zeros(2560, dtype=np.int16)
ended_1, _ = detector.feed(silence_chunk)
ended_2, _ = detector.feed(silence_chunk)
assert not ended_1 and not ended_2, "320ms pause must not prematurely end the turn"
assert detector.in_speech, "Detector must preserve in_speech across 320ms pause"

# Feed continuation speech chunk
detector.feed(chunk_speech)

# Feed sustained silence (> 1.2s = 8 chunks of 160ms)
turn_ended = False
for _ in range(9):
    ended, audio = detector.feed(silence_chunk)
    if ended:
        turn_ended = True
        assert len(audio) > 0
        break

assert turn_ended, "Sustained quiet must conclude turn"
print("OK")
`;

  const proc = spawn(PYTHON_BIN, ['-c', script], {
    cwd: REPO_ROOT,
    env: { ...process.env, PYTHONPATH: REPO_ROOT }
  });

  let stdout = '';
  let stderr = '';
  for await (const chunk of proc.stdout) stdout += chunk;
  for await (const chunk of proc.stderr) stderr += chunk;

  const code = await new Promise((res) => proc.on('close', res));
  assert.equal(code, 0, `Turn detector test failed: ${stderr}`);
  assert.ok(stdout.includes('OK'));
});

test('noise_floor_calibration_adapts_to_ambient_level', async () => {
  const script = `
import numpy as np
from voice.turn_detector import TurnDetector

detector = TurnDetector()
initial_thr = detector.speech_threshold

# Feed quiet ambient noise with low volume (rms ~ 0.002)
low_noise = (np.random.randn(16000) * 60).astype(np.int16)
for i in range(0, len(low_noise), 2560):
    detector.feed_ambient(low_noise[i:i+2560])

thr_low = detector.speech_threshold

# Feed ambient frames with higher background fan noise (rms ~ 0.006)
high_noise = (np.random.randn(16000) * 200).astype(np.int16)
for i in range(0, len(high_noise), 2560):
    detector.feed_ambient(high_noise[i:i+2560])

thr_high = detector.speech_threshold

assert thr_high > thr_low, f"Threshold should adapt upward with higher noise ({thr_high} vs {thr_low})"
print("OK")
`;

  const proc = spawn(PYTHON_BIN, ['-c', script], {
    cwd: REPO_ROOT,
    env: { ...process.env, PYTHONPATH: REPO_ROOT }
  });

  let stdout = '';
  let stderr = '';
  for await (const chunk of proc.stdout) stdout += chunk;
  for await (const chunk of proc.stderr) stderr += chunk;

  const code = await new Promise((res) => proc.on('close', res));
  assert.equal(code, 0, `Noise floor calibration test failed: ${stderr}`);
  assert.ok(stdout.includes('OK'));
});
