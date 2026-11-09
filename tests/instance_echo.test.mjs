import test from 'node:test';
import assert from 'node:assert/strict';
import { runPython } from './python-runner.mjs';

async function assertPythonTest(script, label) {
  const { code, stdout, stderr } = await runPython(script);
  assert.equal(code, 0, `${label} failed: ${stderr}`);
  assert.ok(stdout.includes('OK'));
}

test('single_instance_prevents_duplicate_windows', async () => {
  const script = `
import os
import tempfile
from pathlib import Path
from voice.assistant import acquire_single_instance_lock
from unittest.mock import patch

lock_path = Path("/tmp/leko_assistant_test.pid")
if lock_path.exists():
    lock_path.unlink()

# First acquisition should succeed
with patch("voice.assistant.subprocess.run"), patch("voice.assistant.os.kill"):
    ok1 = acquire_single_instance_lock(lock_file=lock_path)
assert ok1 == True, "First lock acquisition should succeed"
assert lock_path.exists(), "Lock file should be written"
assert lock_path.read_text().strip() == str(os.getpid())

# Clean up
if lock_path.exists():
    lock_path.unlink()
print("OK")
`;

  await assertPythonTest(script, 'Single instance test');
});

test('echo_defense_mutes_during_and_after_playback', async () => {
  const script = `
import time
from voice.assistant import is_audio_muted_for_echo, set_playback_state

# During playback, mic is muted
set_playback_state(True)
assert is_audio_muted_for_echo() == True

# Right after playback stops, 0.7s decay mute is active
set_playback_state(False, post_mute_s=0.7)
assert is_audio_muted_for_echo() == True, "Must stay muted during post-playback decay"

# After decay window, mic is re-armed
time.sleep(0.75)
assert is_audio_muted_for_echo() == False, "Must re-arm mic after decay window"
print("OK")
`;

  await assertPythonTest(script, 'Echo defense test');
});

test('turn_detector_emits_speech_onset_for_ui', async () => {
  const script = `
import numpy as np
from voice.turn_detector import TurnDetector

onsets = []
detector = TurnDetector(speech_detector=False, on_speech_onset=lambda: onsets.append(True))

# 400 Hz tone for speech
t = np.linspace(0, 0.2, 3200, endpoint=False)
speech = (np.sin(2 * np.pi * 400 * t) * 12000).astype(np.int16)

chunk = speech[:2560]
detector.feed(chunk)

assert len(onsets) == 1, "Speech onset callback must be called when voice starts"
print("OK")
`;

  await assertPythonTest(script, 'Speech onset test');
});

test('stop_playback_safely_halts_audio', async () => {
  const script = `
from voice.assistant import stop_playback

# Calling stop_playback even when idle must not throw any exception or AUHAL error
stop_playback()
print("OK")
`;

  await assertPythonTest(script, 'Stop playback test');
});
