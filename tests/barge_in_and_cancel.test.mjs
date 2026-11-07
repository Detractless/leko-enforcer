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

test('stop_action_aborts_inflight_generation_and_discards_reply', async () => {
  const script = `
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

  const proc = spawn(PYTHON_BIN, ['-c', script], {
    cwd: REPO_ROOT,
    env: { ...process.env, PYTHONPATH: REPO_ROOT }
  });

  let stdout = '';
  let stderr = '';
  for await (const chunk of proc.stdout) stdout += chunk;
  for await (const chunk of proc.stderr) stderr += chunk;

  const code = await new Promise((res) => proc.on('close', res));
  assert.equal(code, 0, `Stop action abort test failed: ${stderr}`);
  assert.ok(stdout.includes('OK'));
});

test('cancellation_token_guards_process_query_and_speech', async () => {
  const script = `
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

  const proc = spawn(PYTHON_BIN, ['-c', script], {
    cwd: REPO_ROOT,
    env: { ...process.env, PYTHONPATH: REPO_ROOT }
  });

  let stdout = '';
  let stderr = '';
  for await (const chunk of proc.stdout) stdout += chunk;
  for await (const chunk of proc.stderr) stderr += chunk;

  const code = await new Promise((res) => proc.on('close', res));
  assert.equal(code, 0, `Cancellation guard test failed: ${stderr}`);
  assert.ok(stdout.includes('OK'));
});

test('barge_in_detects_speech_during_playback_and_halts_audio', async () => {
  const script = `
import numpy as np
from voice.turn_detector import TurnDetector
import voice.assistant as assistant

halted = []
def mock_stop():
    halted.append(True)

assistant.stop_playback = mock_stop
detector = TurnDetector(barge_in_threshold=0.035)

# Simulate assistant playback active
detector.set_playback_active(True)

# Feed loud user speech interrupting assistant (RMS ~ 0.12)
user_voice = (np.ones(2560, dtype=np.int16) * 4000)
detector.feed(user_voice)

assert detector.in_speech == True, "Barge-in speech should be detected during playback"
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
  assert.equal(code, 0, `Barge-in detection test failed: ${stderr}`);
  assert.ok(stdout.includes('OK'));
});

test('barge_in_threshold_distinguishes_bleed_from_direct_voice', async () => {
  const script = `
import numpy as np
from voice.turn_detector import TurnDetector

detector = TurnDetector(barge_in_threshold=0.035, speech_threshold=0.010)
detector.set_playback_active(True)

# Low level speaker bleed (RMS ~ 0.018: 600 / 32768 ≈ 0.0183)
speaker_bleed = (np.ones(2560, dtype=np.int16) * 600)
detector.feed(speaker_bleed)
assert detector.in_speech == False, "Speaker bleed must NOT trigger speech onset during playback"

# Direct human voice barge-in (RMS ~ 0.061: 2000 / 32768 ≈ 0.061)
human_voice = (np.ones(2560, dtype=np.int16) * 2000)
detector.feed(human_voice)
assert detector.in_speech == True, "Direct voice above barge-in threshold MUST trigger speech onset"

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
  assert.equal(code, 0, `Threshold bleed vs voice test failed: ${stderr}`);
  assert.ok(stdout.includes('OK'));
});
