import test from 'node:test';
import assert from 'node:assert/strict';
import { CircleController, LekoBridge } from '../ui/web/app.js';
import { MockElement } from './mock-element.mjs';

test('bridge_transfers_intents_and_draft_responses', async () => {
  const stage = new MockElement('circle-stage');
  const status = new MockElement();
  const card = new MockElement();

  const controller = new CircleController(stage, status, card);
  const bridge = new LekoBridge(controller);

  // Send turn
  await bridge.sendVoiceTurn('Start a 30 minute session');

  assert.equal(controller.currentState, 'speaking', 'Transitioned to speaking upon receiving assistant reply');
  assert.ok(controller.currentDraft, 'Controller received the drafted session');
  assert.equal(controller.currentDraft.kind, 'Session');
  assert.equal(controller.currentDraft.name, 'Focus Session');
});
