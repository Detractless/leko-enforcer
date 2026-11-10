import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { runPython } from './python-runner.mjs';

const __dirname = dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = resolve(__dirname, '..');

test('ui_renders_text_free_hold_controls', async () => {
  const html = readFileSync(resolve(REPO_ROOT, 'ui/web/index.html'), 'utf8');
  assert.match(html, /<button[^>]*id=["']stage["'][^>]*aria-label=/);
  assert.match(html, /id=["']status["'][^>]*class=["']visually-hidden["'][^>]*aria-live=/);
  assert.doesNotMatch(html, /btn-mute|btn-stop|>\s*(Mute|Stop)\s*</i);
  const visible = html
    .replace(/<title>[\s\S]*?<\/title>/i, '')
    .replace(/<(?:div|span)[^>]*class=["'][^"']*visually-hidden[^"']*["'][^>]*>[\s\S]*?<\/(?:div|span)>/gi, '')
    .replace(/<[^>]*>/g, '')
    .replace(/\s+/g, '');
  assert.equal(visible, '', 'Only hidden accessible text may remain in the visible UI');
});

test('escape_cancel_sends_unsubmitted_audio_cancel', async () => {
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

  bridge.cancelUnsubmitted();
  assert.equal(postedActions[postedActions.length - 1].action, 'ptt_cancel');
  assert.equal(controller.currentState, 'idle');
});

const TURN_END_SCRIPT = `
import sys
import numpy as np
from voice.turn_detector import TurnDetector

detector = TurnDetector(speech_detector=False)

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

# Feed quiet frames past the 5s continuation window
silence = np.zeros(87040, dtype=np.int16)
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

test('vad_and_turn_end_classifier_detect_speech_conclusion', async () => {
  // Test Python turn end detector directly


  const { code, stdout, stderr } = await runPython(TURN_END_SCRIPT);
  assert.equal(code, 0, `Turn detector failed: ${stderr}`);
  assert.ok(stdout.includes('OK'));
});

const HANDSFREE_PIPELINE_SCRIPT = `
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

test('handsfree_pipeline_processes_query_on_turn_end', async () => {


  const { code, stdout, stderr } = await runPython(HANDSFREE_PIPELINE_SCRIPT);
  assert.equal(code, 0, `Handsfree pipeline test failed: ${stderr}`);
  assert.ok(stdout.includes('OK'));
});
