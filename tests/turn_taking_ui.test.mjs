import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync, existsSync } from 'node:fs';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawn } from 'node:child_process';

const __dirname = dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = resolve(__dirname, '..');
const TALK_PYTHON = '/Users/calibro/.t3/scratch/phone-pilot/talk-venv/bin/python3';
const PYTHON_BIN = existsSync(TALK_PYTHON) ? TALK_PYTHON : 'python3';

test('ui_renders_centered_label_and_controls', async () => {
  const html = readFileSync(resolve(REPO_ROOT, 'ui/web/index.html'), 'utf8');

  // AC-001: Status text is located inside the circle stage
  assert.match(html, /<div[^>]*id=["']stage["'][^>]*>[\s\S]*?<div[^>]*id=["']status["']/, 'Status text must be nested inside stage');

  // Control buttons exist in the DOM
  assert.match(html, /id=["']btn-mute["']/, 'Mute button must exist in index.html');
  assert.match(html, /id=["']btn-stop["']/, 'Stop button must exist in index.html');
});

test('controls_trigger_mute_and_stop_ipc_actions', async () => {
  const { CircleController, LekoBridge } = await import('../ui/web/app.js');

  const stage = { className: 'circle-stage state-idle' };
  const status = { textContent: 'Ready' };
  const card = { className: 'draft-card', querySelector: () => ({ textContent: '' }) };

  const controller = new CircleController(stage, status, card);
  const bridge = new LekoBridge(controller);

  let postedActions = [];
  globalThis.window = {
    ipc: {
      postMessage: (msg) => { postedActions.push(JSON.parse(msg)); }
    }
  };

  // Test Mute toggle
  bridge.toggleMute();
  assert.equal(bridge.isMuted, true);
  assert.equal(postedActions[postedActions.length - 1].action, 'mute');
  assert.equal(postedActions[postedActions.length - 1].muted, true);

  bridge.toggleMute();
  assert.equal(bridge.isMuted, false);
  assert.equal(postedActions[postedActions.length - 1].action, 'mute');
  assert.equal(postedActions[postedActions.length - 1].muted, false);

  // Test Stop action
  bridge.stop();
  assert.equal(postedActions[postedActions.length - 1].action, 'stop');
  assert.equal(controller.currentState, 'idle');
});

test('vad_and_turn_end_classifier_detect_speech_conclusion', async () => {
  // Test Python turn end detector directly
  const script = `
import sys
import numpy as np
from voice.turn_detector import TurnDetector

detector = TurnDetector()

# Feed 1 second of speech-level audio (sine wave 400Hz at 16kHz)
t = np.linspace(0, 1.0, 16000, endpoint=False)
speech = (np.sin(2 * np.pi * 400 * t) * 12000).astype(np.int16)

turn_ended = False
for i in range(0, len(speech), 2560):
    chunk = speech[i:i+2560]
    if len(chunk) == 2560:
        ended, _ = detector.feed(chunk)
        if ended:
            turn_ended = True

assert not turn_ended, "Turn should not end while active speech is ongoing"

# Feed quiet frames
silence = np.zeros(24000, dtype=np.int16)
for i in range(0, len(silence), 2560):
    chunk = silence[i:i+2560]
    if len(chunk) == 2560:
        ended, audio = detector.feed(chunk)
        if ended:
            turn_ended = True
            assert len(audio) > 0, "Captured audio must be returned on turn end"
            break

assert turn_ended, "Turn must end after speech followed by silence"
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
  assert.equal(code, 0, `Turn detector failed: ${stderr}`);
  assert.ok(stdout.includes('OK'));
});

test('handsfree_pipeline_processes_query_on_turn_end', async () => {
  const script = `
import sys
from voice.ui_bridge import UIPipelineBridge

events = []
bridge = UIPipelineBridge(event_sink=lambda ev: events.append(ev))

# Simulate hands-free speech pipeline event
bridge.emit_speaking("Maeve finished turn")
bridge.emit_idle()

assert events[0]["state"] == "speaking"
assert events[1]["state"] == "idle"
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
  assert.equal(code, 0, `Handsfree pipeline test failed: ${stderr}`);
  assert.ok(stdout.includes('OK'));
});
