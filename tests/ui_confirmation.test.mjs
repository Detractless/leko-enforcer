import test from 'node:test';
import assert from 'node:assert/strict';
import { CircleController } from '../ui/web/app.js';
import { MockElement } from './mock-element.mjs';

test('draft_card_renders_and_handles_confirm_actions', () => {
  const stage = new MockElement('circle-stage');
  const status = new MockElement();
  const card = new MockElement('draft-card');

  const controller = new CircleController(stage, status, card);

  let confirmedDraft = null;
  let cancelledDraft = null;

  const sampleDraft = {
    name: 'Reddit Block',
    kind: 'Block',
    description: 'Closes reddit.com for 2 hours.'
  };

  // 1. Show draft
  controller.showDraft(
    sampleDraft,
    (draft) => { confirmedDraft = draft; },
    (draft) => { cancelledDraft = draft; }
  );

  assert.ok(card.classList.contains('visible'), 'Card is visible');
  assert.equal(card.querySelector('.card-title').textContent, 'Reddit Block');
  assert.equal(card.querySelector('.card-badge').textContent, 'Block');
  assert.equal(card.querySelector('.card-details').textContent, 'Closes reddit.com for 2 hours.');

  // 2. Confirm draft
  controller.confirmDraft();
  assert.equal(confirmedDraft, sampleDraft, 'Confirm callback received the draft');
  assert.equal(card.classList.contains('visible'), false, 'Card is hidden on confirm');
  assert.equal(controller.currentState, 'thinking', 'Waits for backend confirmation');

  // 3. Test cancel path
  controller.showDraft(
    sampleDraft,
    (d) => {},
    (d) => { cancelledDraft = d; }
  );
  assert.ok(card.classList.contains('visible'));
  controller.cancelDraft();
  assert.equal(cancelledDraft, sampleDraft, 'Cancel callback received the draft');
  assert.equal(card.classList.contains('visible'), false, 'Card is hidden on cancel');
  assert.equal(controller.currentState, 'idle', 'Circle resets to idle on cancel');
});
