import test from 'node:test';
import assert from 'node:assert/strict';
import { CircleController } from '../ui/web/app.js';
import { MockElement } from './mock-element.mjs';

test('circle_transitions_across_all_voice_states', () => {
  const stage = new MockElement('circle-stage');
  const status = new MockElement();
  const card = new MockElement();

  const controller = new CircleController(stage, status, card);

  // 1. Idle state
  controller.setState('idle');
  assert.equal(controller.currentState, 'idle');
  assert.equal(stage.className, 'circle-stage state-idle');
  assert.equal(status.textContent, 'Ready');

  // 2. Listening state
  controller.setState('listening');
  assert.equal(controller.currentState, 'listening');
  assert.equal(stage.className, 'circle-stage state-listening');
  assert.equal(status.textContent, 'Listening...');

  // 3. Thinking state
  controller.setState('thinking');
  assert.equal(controller.currentState, 'thinking');
  assert.equal(stage.className, 'circle-stage state-thinking');
  assert.equal(status.textContent, 'Thinking...');

  // 4. Speaking state
  controller.setState('speaking');
  assert.equal(controller.currentState, 'speaking');
  assert.equal(stage.className, 'circle-stage state-speaking');
  assert.equal(status.textContent, 'Maeve Speaking...');

  // 5. Confirmed state
  controller.setState('confirmed');
  assert.equal(controller.currentState, 'confirmed');
  assert.equal(stage.className, 'circle-stage state-confirmed');
  assert.equal(status.textContent, 'Locked & Saved');

  // Invalid state rejection
  assert.throws(() => {
    controller.setState('unknown_state');
  }, /Invalid circle state/);
});
