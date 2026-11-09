import test from 'node:test';
import assert from 'node:assert/strict';
import { runPythonSync } from './python-runner.mjs';

test('WebRTC VAD handles 20ms frames, onset, and safe failure', () => {
  const script = `
import sys, types, numpy as np
frames = []
class FakeVad:
    def __init__(self, mode): assert mode == 2
    def is_speech(self, frame, rate):
        assert len(frame) == 640 and rate == 16000
        frames.append(frame)
        return len(frames) == 2
sys.modules['webrtcvad'] = types.SimpleNamespace(Vad=FakeVad)
from voice.audio_io.webrtc_vad import WebRTCVAD
from voice.turn_detector import TurnDetector
vad = WebRTCVAD()
assert vad.process(np.zeros(100, dtype=np.float32)) is None
assert vad.process(np.zeros(220, dtype=np.float32)) == 0.0
assert vad.process(np.zeros(320, dtype=np.float32)) == 1.0
vad.reset()
onsets = []
detector = TurnDetector(speech_detector=vad, on_speech_onset=lambda: onsets.append(1))
frames.clear()
detector.feed(np.zeros(2560, dtype=np.int16))
assert onsets == [1], 'A positive speech frame should trigger onset'
detector.reset()
detector.feed(np.array([float('nan')], dtype=np.float32))
assert vad._failed and detector.speech_detector is vad
detector.feed(np.ones(2560, dtype=np.int16) * 20000)
assert onsets == [1], 'A failed WebRTC detector must not fall back silently'
`;
  const result = runPythonSync(['-c', script]);
  assert.equal(result.status, 0, result.stderr);
});

test('unavailable WebRTC VAD exits before starting the app', () => {
  const script = `
import builtins, sys
sys.path.insert(0, 'voice')
import session.runtime as runtime
original = builtins.__import__
def no_vad(name, *args, **kwargs):
    if name == 'webrtcvad': raise ImportError('test dependency missing')
    return original(name, *args, **kwargs)
builtins.__import__ = no_vad
runtime.voice.acquire_single_instance_lock = lambda: (_ for _ in ()).throw(AssertionError('started'))
sys.argv = ['assistant', '--speech-detector', 'webrtc']
try: runtime.main()
except SystemExit as exc: assert exc.code == 2
else: raise AssertionError('missing dependency must stop launch')
`;
  const result = runPythonSync(['-c', script]);
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stderr, /WebRTC VAD unavailable/);
});
