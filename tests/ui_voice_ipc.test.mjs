import test from 'node:test';
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = resolve(__dirname, '..');

// Minimal mock DOM for headless testing of UI controller and bridge
function createMockDOM() {
  const listeners = {};
  const mockClassList = (el) => ({
    classes: new Set(),
    add: (cls) => el.className += ' ' + cls,
    remove: (cls) => el.className = el.className.replace(new RegExp('\\b' + cls + '\\b', 'g'), '').trim(),
    contains: (cls) => el.className.split(/\s+/).includes(cls)
  });

  const stage = {
    className: 'circle-stage state-idle',
    classList: null,
    addEventListener: (ev, fn) => { listeners[ev] = fn; }
  };
  stage.classList = mockClassList(stage);

  const status = { textContent: 'Ready' };

  const titleEl = { textContent: '' };
  const badgeEl = { textContent: '' };
  const detailsEl = { textContent: '' };
  const card = {
    className: 'draft-card',
    classList: null,
    querySelector: (sel) => {
      if (sel === '.card-title') return titleEl;
      if (sel === '.card-badge') return badgeEl;
      if (sel === '.card-details') return detailsEl;
      return null;
    }
  };
  card.classList = mockClassList(card);

  return { stage, status, card, titleEl, badgeEl, detailsEl };
}

test('voice_pipeline_emits_lifecycle_events_to_ipc', async () => {
  // Test Python voice assistant UI IPC bridge protocol
  const script = `
import sys
import json
from voice.ui_bridge import UIPipelineBridge

events = []
bridge = UIPipelineBridge(event_sink=lambda ev: events.append(ev))

# Simulate query lifecycle through bridge
bridge.emit_listening()
bridge.emit_thinking("Block youtube for 1 hour")
bridge.emit_draft(name="YouTube Block", kind="Block", description="Block youtube.com for 1 hour")
bridge.emit_speaking("Drafted a block for youtube.com.")
bridge.emit_confirmed()

print(json.dumps(events))
`;

  const proc = spawn('python3', ['-c', script], {
    cwd: REPO_ROOT,
    env: { ...process.env, PYTHONPATH: REPO_ROOT }
  });

  let stdout = '';
  let stderr = '';
  for await (const chunk of proc.stdout) stdout += chunk;
  for await (const chunk of proc.stderr) stderr += chunk;

  const code = await new Promise((res) => proc.on('close', res));
  assert.equal(code, 0, `Python script failed: ${stderr}`);

  const events = JSON.parse(stdout.trim());
  assert.equal(events.length, 5);
  assert.equal(events[0].type, 'state');
  assert.equal(events[0].state, 'listening');

  assert.equal(events[1].type, 'state');
  assert.equal(events[1].state, 'thinking');

  assert.equal(events[2].type, 'draft');
  assert.equal(events[2].draft.name, 'YouTube Block');

  assert.equal(events[3].type, 'state');
  assert.equal(events[3].state, 'speaking');

  assert.equal(events[4].type, 'state');
  assert.equal(events[4].state, 'confirmed');
});

test('ui_bridge_updates_circle_state_on_ipc_event', async () => {
  const { CircleController, LekoBridge } = await import('../ui/web/app.js');
  const dom = createMockDOM();
  const controller = new CircleController(dom.stage, dom.status, dom.card);
  const bridge = new LekoBridge(controller);

  // Wire up incoming IPC handler
  bridge.handleIncomingEvent({ type: 'state', state: 'listening' });
  assert.equal(controller.currentState, 'listening');
  assert.ok(dom.stage.className.includes('state-listening'));
  assert.equal(dom.status.textContent, 'Listening...');

  bridge.handleIncomingEvent({ type: 'state', state: 'thinking' });
  assert.equal(controller.currentState, 'thinking');
  assert.ok(dom.stage.className.includes('state-thinking'));

  bridge.handleIncomingEvent({ type: 'state', state: 'speaking', label: 'Maeve Speaking...' });
  assert.equal(controller.currentState, 'speaking');
  assert.ok(dom.stage.className.includes('state-speaking'));
  assert.equal(dom.status.textContent, 'Maeve Speaking...');
});

test('draft_event_renders_card_and_transmits_user_action', async () => {
  const { CircleController, LekoBridge } = await import('../ui/web/app.js');
  const dom = createMockDOM();
  const controller = new CircleController(dom.stage, dom.status, dom.card);
  const bridge = new LekoBridge(controller);

  let postedIpcMessage = null;
  globalThis.window = {
    ipc: {
      postMessage: (msg) => { postedIpcMessage = JSON.parse(msg); }
    }
  };

  bridge.handleIncomingEvent({
    type: 'draft',
    draft: {
      name: 'Focus Session',
      kind: 'Session',
      description: 'Immediate focus session for 30 minutes.'
    }
  });

  // Verify card DOM updated and made visible
  assert.ok(dom.card.className.includes('visible'));
  assert.equal(dom.titleEl.textContent, 'Focus Session');
  assert.equal(dom.badgeEl.textContent, 'Session');

  // Trigger confirmation
  controller.confirmDraft();
  assert.notEqual(postedIpcMessage, null);
  assert.equal(postedIpcMessage.action, 'confirm');
  assert.equal(postedIpcMessage.draft.name, 'Focus Session');
  assert.equal(controller.currentState, 'confirmed');
});

test('native_launcher_handles_bidirectional_stdio_ipc', async () => {
  // Test JSON line parsing and malformed input handling
  const { parseIpcLine, formatIpcMessage } = await import('../ui/web/app.js');

  const valid = parseIpcLine('{"type":"state","state":"listening"}\n');
  assert.deepEqual(valid, { type: 'state', state: 'listening' });

  // Malformed JSON must not crash, should return null
  const invalid = parseIpcLine('not-json-broken-line\n');
  assert.equal(invalid, null);

  const formatted = formatIpcMessage({ action: 'confirm', target: 'rule-1' });
  assert.equal(formatted.trim(), '{"action":"confirm","target":"rule-1"}');
});
