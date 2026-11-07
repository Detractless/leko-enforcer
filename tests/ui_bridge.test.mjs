import test from 'node:test';
import assert from 'node:assert/strict';
import { CircleController, LekoBridge } from '../ui/web/app.js';

class MockElement {
  constructor(className = '') {
    this.className = className;
    this.textContent = '';
    this.classList = {
      add: (c) => { if (!this.className.includes(c)) this.className += ` ${c}`; },
      remove: (c) => { this.className = this.className.replace(c, '').trim(); },
      contains: (c) => this.className.includes(c)
    };
  }
  querySelector() {
    return new MockElement();
  }
}

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
