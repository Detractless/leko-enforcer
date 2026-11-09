import test from 'node:test';
import assert from 'node:assert/strict';
import { existsSync } from 'node:fs';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { runPython } from './python-runner.mjs';

const __dirname = dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = resolve(__dirname, '..');
const AEC_FILENAME = process.platform === 'win32'
  ? 'leko_audio.dll'
  : process.platform === 'darwin' ? 'libleko_audio.dylib' : 'libleko_audio.so';
const AEC_LIBRARY = resolve(REPO_ROOT, 'target/debug', AEC_FILENAME);

const ECHO_BRIDGE_SCRIPT = `
import numpy as np
import sounddevice as sd
import threading
from voice.audio_io.echo_canceller import EchoAudioProcessor
from voice.audio_io.echo_canceller import VoiceOutputPlayer

aec = EchoAudioProcessor()
assert aec.available, "Native Rust echo canceller should load"
frame_size = 160
delay = 320
frames = 600
random = np.random.default_rng(81)
render = random.uniform(-0.25, 0.25, frames * frame_size).astype(np.float32)
residual = input_energy = 0.0

for frame_index in range(frames):
    start = frame_index * frame_size
    reference = render[start:start + frame_size]
    positions = np.arange(start, start + frame_size) - delay
    capture = np.where(positions >= 0, render[np.maximum(positions, 0)] * 0.6, 0.0)
    assert aec.feed_render(reference)
    cleaned = aec.clean_capture((capture * 32767).astype(np.int16))
    assert cleaned is not None
    if frame_index >= 400:
        residual += float(np.square(cleaned.astype(np.float32) / 32768).sum())
        input_energy += float(np.square(capture).sum())

assert residual ** 0.5 < input_energy ** 0.5 * 0.5, "Bridge should cancel more than half of delayed echo"

rendered_frames = []
feed_render = aec.feed_render
def record_render(samples):
    rendered_frames.extend(samples.tolist())
    return feed_render(samples)
aec.feed_render = record_render

class FakeOutputStream:
    def __init__(self, samplerate, channels, dtype, callback):
        self.callback = callback
        self.active = False
    def start(self):
        self.active = True
        while self.active:
            output = np.zeros((480, 1), dtype=np.float32)
            try:
                self.callback(output, len(output), None, None)
            except sd.CallbackStop:
                self.active = False
            except sd.CallbackAbort:
                self.active = False
                raise AssertionError("Reference callback failed")
    def stop(self):
        self.active = False
    def abort(self):
        self.active = False
    def close(self):
        self.active = False

sd.OutputStream = FakeOutputStream
player = VoiceOutputPlayer(aec)
assert player.play(np.ones(2400, dtype=np.float32) * 0.1, 24_000)
assert len(rendered_frames) >= 160, "Speaker callback should provide echo reference frames"

# An interruption during setup must prevent playback and must not trigger fallback.
entered = threading.Event()
release = threading.Event()
original_reset = aec.reset_buffers
def slow_reset():
    entered.set()
    assert release.wait(3)
    original_reset()
aec.reset_buffers = slow_reset
starts = []
original_start = FakeOutputStream.start
def record_start(stream):
    starts.append(True)
    original_start(stream)
FakeOutputStream.start = record_start
result = []
thread = threading.Thread(target=lambda: result.append(player.play(np.ones(2400), 24_000)))
thread.start()
assert entered.wait(3)
player.stop()
release.set()
thread.join(3)
assert not thread.is_alive()
assert result == [True], "Cancelled playback must not fall back and replay"
assert not starts, "A stop during setup must prevent playback from starting"
aec.close()
print("OK")
`;

test('python_audio_bridge_cancels_delayed_synthetic_echo', { skip: !existsSync(AEC_LIBRARY) }, async () => {

  const { code, stdout, stderr } = await runPython(ECHO_BRIDGE_SCRIPT);
  assert.equal(code, 0, `Python/Rust echo bridge test failed: ${stderr}`);
  assert.ok(stdout.includes('OK'));
});
