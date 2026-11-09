import test from 'node:test';
import assert from 'node:assert/strict';
import { DETECTOR_FAILURE_TAIL, expectDetectorDisabled, runArgs, runCode } from './onset-detector-scripts.mjs';

test('RNNoise processes silence, resets, and disables listening on failure', () => {
  const code = `
import numpy as np
from voice.audio_io.rnnoise_vad import RNNoiseVAD
from voice.turn_detector import TurnDetector
vad = RNNoiseVAD()
assert all(vad.process(np.zeros(2560, dtype=np.float32)) < 0.5 for _ in range(4))
vad.reset()
onsets = []
turn = TurnDetector(speech_detector=vad, on_speech_onset=lambda: onsets.append(True))
turn.feed(np.zeros(2560, dtype=np.int16))
assert not onsets
assert vad.process(np.array([float('nan')], dtype=np.float32)) == 0.0
${DETECTOR_FAILURE_TAIL}
`;
  expectDetectorDisabled(runCode(code));
});

test('RNNoise is selectable without starting the microphone', () => {
  const result = runArgs(['voice/assistant.py', '--help']);
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /--speech-detector \{silero,loudness,webrtc,rnnoise,ten\}/);
});

test('missing RNNoise exits before starting the assistant', () => {
  const code = `
import importlib.metadata, sys
sys.path.insert(0, 'voice')
import session.runtime as runtime
def missing(name):
    raise importlib.metadata.PackageNotFoundError(name)
importlib.metadata.distribution = missing
runtime.voice.acquire_single_instance_lock = lambda: (_ for _ in ()).throw(AssertionError('started'))
sys.argv = ['assistant', '--speech-detector', 'rnnoise']
try: runtime.main()
except SystemExit as exc: assert exc.code == 2
else: raise AssertionError('missing dependency must stop launch')
`;
  const result = runCode(code);
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stderr, /RNNoise unavailable/);
});
