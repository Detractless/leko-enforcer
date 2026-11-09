import test from 'node:test';
import assert from 'node:assert/strict';
import { DETECTOR_FAILURE_TAIL, expectDetectorDisabled, runArgs, runCode } from './onset-detector-scripts.mjs';

test('TEN processes audio frames and disables listening after a detector failure', () => {
  const code = `
import numpy as np
from voice.audio_io.ten_vad import TENVAD
from voice.turn_detector import TurnDetector
vad = TENVAD()
assert vad.process(np.zeros(100, dtype=np.float32)) is None
assert vad.process(np.zeros(156, dtype=np.float32)) < 0.5
vad.reset()
onsets = []
turn = TurnDetector(speech_detector=vad, on_speech_onset=lambda: onsets.append(True))
turn.feed(np.zeros(2560, dtype=np.int16))
assert not onsets
vad._lib.ten_vad_process = lambda *args: -1
assert vad.process(np.zeros(256, dtype=np.float32)) == 0.0
${DETECTOR_FAILURE_TAIL}
`;
  expectDetectorDisabled(runCode(code));
});

test('TEN is selectable and a missing library exits before assistant startup', () => {
  const help = runArgs(['voice/assistant.py', '--help']);
  assert.equal(help.status, 0, help.stderr);
  assert.match(help.stdout, /--speech-detector \{silero,loudness,webrtc,rnnoise,ten\}/);
  const code = `
import sys
sys.path.insert(0, 'voice')
import session.runtime as runtime
import audio_io.ten_vad as ten
ten.LIBRARY = '/nonexistent/ten-vad.dylib'
ten.TENVAD.__init__.__defaults__ = (ten.LIBRARY,)
runtime.voice.acquire_single_instance_lock = lambda: (_ for _ in ()).throw(AssertionError('started'))
sys.argv = ['assistant', '--speech-detector', 'ten']
try: runtime.main()
except SystemExit as exc: assert exc.code == 2
else: raise AssertionError('missing library must stop launch')
`;
  const result = runCode(code);
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stderr, /TEN VAD unavailable/);
});
