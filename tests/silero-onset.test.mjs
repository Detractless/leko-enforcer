import test from 'node:test';
import assert from 'node:assert/strict';
import { existsSync } from 'node:fs';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawn } from 'node:child_process';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const venv = '/Users/calibro/.t3/scratch/phone-pilot/talk-venv/bin/python3';

test('speech detection rejects noise, preserves lead-in, and handles failure', async () => {
  const script = `
import numpy as np
from voice.turn_detector import TurnDetector

class FakeDetector:
    def __init__(self, probability=0.0):
        self.probability = probability
        self.calls = 0
        self.resets = 0
    def process(self, samples):
        self.calls += 1
        return self.probability
    def reset(self):
        self.resets += 1

fake = FakeDetector()
onsets = []
detector = TurnDetector(speech_detector=fake, on_speech_onset=lambda: onsets.append(True))
noise = np.ones(2560, dtype=np.int16) * 12000
detector.feed(noise)
assert not detector.in_speech and not onsets, 'Loud noise should not trigger when VAD rejects it'
assert fake.calls > 0
lead_in = np.ones(2560, dtype=np.int16) * 150
detector.feed(lead_in)
fake.probability = 1.0
speech = np.ones(2560, dtype=np.int16) * 4000
detector.feed(speech)
assert detector.in_speech and len(onsets) == 1
assert len(detector.buf) >= 2, 'Speech onset must retain earlier audio'
assert np.array_equal(detector.buf[-2], lead_in.astype(np.float32) / 32768.0)

# In half-duplex, playback strictly mutes mic; user interrupts via UI/Space.
fake = FakeDetector()
onsets = []
detector = TurnDetector(speech_detector=fake, on_speech_onset=lambda: onsets.append(True))
detector.set_echo_cancellation_enabled(True)
detector.set_playback_active(True)
detector.feed(noise)
assert not onsets, 'Noise must not trigger during playback'
fake.probability = 1.0
detector.feed(speech)
assert not onsets, 'Mic is muted during playback to avoid speaker feedback'
detector.set_playback_active(False, post_mute_s=0.0)
detector.feed(speech)
assert len(onsets) == 1, 'Direct voice re-arms after playback completes'

# A failing VAD must use the existing loudness fallback instead of crashing.
class FailingDetector(FakeDetector):
    def process(self, samples):
        raise RuntimeError('simulated model failure')

onsets = []
detector = TurnDetector(speech_detector=FailingDetector(), on_speech_onset=lambda: onsets.append(True))
detector.feed(speech)
assert detector.in_speech and len(onsets) == 1, 'VAD failure must fall back to loudness detection'
print('OK')
`;
  const proc = spawn(existsSync(venv) ? venv : 'python3', ['-c', script], {
    cwd: root, env: { ...process.env, PYTHONPATH: root }
  });
  let stderr = '';
  for await (const chunk of proc.stderr) stderr += chunk;
  const code = await new Promise(resolve => proc.on('close', resolve));
  assert.equal(code, 0, `Speech onset detection failed: ${stderr}`);
});
