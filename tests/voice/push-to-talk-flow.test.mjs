import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import vm from 'node:vm';
import { CircleController, LekoBridge } from '../ui/web/app.js';

const root = new URL('..', import.meta.url);
const html = readFileSync(new URL('ui/web/index.html', root), 'utf8');
const inputSource = readFileSync(new URL('ui/web/circle-controller.js', root), 'utf8');

test.afterEach(() => { delete globalThis.window; });

class ElementStub {
  constructor(id = '', tagName = 'DIV') {
    this.id = id;
    this.tagName = tagName;
    this.className = '';
    this.textContent = '';
    this.attributes = {};
    this.listeners = {};
    this.children = {};
    this.classList = {
      add: (name) => { if (!this.className.split(/\s+/).includes(name)) this.className += ` ${name}`; },
      remove: (name) => { this.className = this.className.split(/\s+/).filter((v) => v !== name).join(' '); },
      contains: (name) => this.className.split(/\s+/).includes(name)
    };
  }
  addEventListener(name, fn) { this.listeners[name] = fn; }
  dispatch(name, event = {}) { this.listeners[name]?.({ target: this, preventDefault() {}, ...event }); }
  setPointerCapture(pointerId) { this.capturedPointer = pointerId; }
  setAttribute(name, value) { this.attributes[name] = value; }
  getAttribute(name) { return this.attributes[name]; }
  closest() { return ['INPUT', 'TEXTAREA', 'SELECT'].includes(this.tagName) || this.id.startsWith('btn-') ? this : null; }
  querySelector(selector) { return this.children[selector] ||= new ElementStub(selector); }
}

function mountUi() {
  const actions = [];
  const nodes = {
    stage: new ElementStub('stage', 'BUTTON'),
    status: new ElementStub('status'),
    'draft-card': new ElementStub('draft-card'),
    'btn-confirm': new ElementStub('btn-confirm', 'BUTTON'),
    'btn-cancel': new ElementStub('btn-cancel', 'BUTTON')
  };
  const window = {
    handlers: {},
    ipc: { postMessage: (message) => actions.push(JSON.parse(message)) },
    addEventListener(name, fn) { this.handlers[name] = fn; },
    dispatch(name, event = {}) { this.handlers[name]?.({ preventDefault() {}, ...event }); }
  };
  const document = {
    hidden: false,
    handlers: {},
    getElementById: (id) => nodes[id],
    addEventListener(name, fn) { this.handlers[name] = fn; }
  };
  globalThis.window = window;
  window.CircleController = CircleController;
  window.LekoBridge = LekoBridge;
  vm.runInNewContext(inputSource, { window, document, Element: ElementStub });
  return { actions, document, nodes, window };
}

test('circle_pointer_hold_captures_until_release_outside', () => {
  const ui = mountUi();
  ui.nodes.stage.dispatch('pointerdown', { button: 0, pointerId: 7 });
  assert.equal(ui.nodes.stage.capturedPointer, 7);
  assert.deepEqual(ui.actions.map((action) => action.action), ['ptt_start']);
  ui.nodes.stage.dispatch('pointerup', { pointerId: 7, target: new ElementStub('outside') });
  assert.deepEqual(ui.actions.map((action) => action.action), ['ptt_start', 'ptt_end']);
  assert.ok(ui.nodes.stage.className.includes('state-grace'));
  delete globalThis.window;
});

test('space_hold_matches_pointer_without_repeat_or_field_capture', () => {
  const ui = mountUi();
  ui.window.dispatch('keydown', { code: 'Space', repeat: false, target: ui.nodes.stage });
  ui.window.dispatch('keydown', { code: 'Space', repeat: true, target: ui.nodes.stage });
  ui.window.dispatch('keydown', { code: 'Space', repeat: false, target: new ElementStub('field', 'INPUT') });
  ui.window.dispatch('keyup', { code: 'Space', target: ui.nodes.stage });
  assert.deepEqual(ui.actions.map((action) => action.action), ['ptt_start', 'ptt_end']);
  delete globalThis.window;
});

test('escape_cancels_unsubmitted_audio_even_when_a_draft_control_has_focus', () => {
  const ui = mountUi();
  ui.window.dispatch('keydown', { code: 'Space', repeat: false, target: ui.nodes.stage });
  ui.window.dispatch('keydown', { code: 'Escape', target: ui.nodes['btn-confirm'] });
  assert.deepEqual(ui.actions.map((action) => action.action), ['ptt_start', 'ptt_cancel']);
  assert.ok(ui.nodes.stage.className.includes('state-idle'));
  delete globalThis.window;
});

test('ui_has_no_visible_text_and_announces_each_state', () => {
  assert.match(html, /aria-live="polite"/);
  assert.match(html, /aria-label="Hold to speak/);
  assert.doesNotMatch(html, /btn-mute|btn-stop/);
  const visibleText = html.replace(/<title>[\s\S]*?<\/title>/i, '')
    .replace(/<(?:div|span)[^>]*class="[^"]*visually-hidden[^"]*"[^>]*>[\s\S]*?<\/(?:div|span)>/gi, '')
    .replace(/<[^>]*>/g, '').replace(/\s+/g, '');
  assert.equal(visibleText, '');

  const ui = mountUi();
  for (const state of ['held', 'grace', 'thinking', 'speaking', 'error']) {
    ui.window.__leko_on_event({ type: 'state', state });
    assert.ok(ui.nodes.stage.className.includes(`state-${state}`));
    assert.ok(ui.nodes.status.textContent.length > 0);
  }
  delete globalThis.window;
});

test('pending_action_details_and_icon_controls_remain_accessible', () => {
  assert.match(html, /aria-describedby="draft-details"/);
  assert.match(html, /id="draft-details" class="visually-hidden card-details"/);
  assert.match(html, /aria-label="Confirm pending action"/);
  assert.match(html, /aria-label="Cancel pending action"/);
  const ui = mountUi();
  ui.window.__leko_on_event({
    type: 'draft',
    draft: { name: 'Device block', kind: 'Block', description: 'Blocks the whole computer from midnight to 7 a.m.' }
  });
  const card = ui.nodes['draft-card'];
  assert.equal(card.getAttribute('aria-hidden'), 'false');
  assert.equal(card.querySelector('.card-details').textContent, 'Blocks the whole computer from midnight to 7 a.m.');
  delete globalThis.window;
});
