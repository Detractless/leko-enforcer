import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { runInNewContext } from 'node:vm';

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '../..');

function mountPillVm() {
  const js = readFileSync(resolve(ROOT, 'ui/src/web/pill.js'), 'utf8');
  const listeners = new Map();
  const makeEl = (id) => ({
    style: { setProperty() {} },
    classList: { add() {}, remove() {} },
    children: [],
    appendChild(c) { this.children.push(c); },
    addEventListener(type, fn) { listeners.set(`${id}:${type}`, fn); },
    contains() { return false; },
    setPointerCapture() {},
    hidden: false,
    textContent: '',
    innerHTML: '',
  });
  const els = Object.fromEntries(
    ['pill', 'bars', 'label', 'card', 'card-close', 'card-body', 'card-table', 'card-head', 'card-rows']
      .map((k) => [k, makeEl(k)]),
  );
  let now = 1000;
  const timers = [];
  const posted = [];
  const ctx = {
    document: {
      getElementById: (id) => els[id] || makeEl(id),
      createElement: () => makeEl('el'),
      addEventListener() {},
      body: { classList: { add() {}, remove() {} } },
    },
    window: {
      addEventListener() {},
      ipc: { postMessage(msg) { posted.push(JSON.parse(msg).action); } },
      __leko_config: {},
    },
    performance: { now: () => now },
    requestAnimationFrame: () => 0,
    setTimeout: (fn, ms) => {
      const entry = { fn, at: now + ms, ms, cleared: false, fired: false };
      timers.push(entry);
      return entry;
    },
    clearTimeout: (entry) => { if (entry) { entry.cleared = true; } },
  };
  ctx.window.window = ctx.window;
  runInNewContext(js, ctx);
  return {
    els,
    listeners,
    posted,
    timers,
    ctx,
    advance(entry) {
      now = entry.at;
      entry.fired = true;
      entry.fn();
    },
  };
}

test('word_timings_drive_word_reveal_and_pointerdown_dismisses_active_card', () => {
  const vm = mountPillVm();
  vm.ctx.window.__leko_on_event({ type: 'reply_card', text: 'Alpha beta.' });
  vm.ctx.window.__leko_on_event({
    type: 'state', state: 'speaking', text: 'Alpha beta.', duration_ms: 2000, word_timings_ms: [160, 820],
  });
  assert.equal(vm.els['card-body'].textContent, '');
  const active = vm.timers.filter((t) => !t.cleared && !t.fired);
  assert.equal(active.length, 1);
  assert.equal(active[0].ms, 160);
  vm.advance(active[0]);
  assert.equal(vm.els['card-body'].textContent, 'Alpha ');
  vm.listeners.get('pill:pointerdown')({ pointerId: 1 });
  assert.equal(vm.els.card.hidden, true);
  assert.deepEqual(vm.posted.slice(-2), ['ptt_start', 'card_dismiss']);
  assert.equal(vm.timers.some((t) => !t.cleared && !t.fired), false);
});

test('letter_reveal_mode_types_individual_characters_across_word_windows', () => {
  const vm = mountPillVm();
  vm.ctx.window.__leko_on_event({ type: 'reply_card', text: 'No go.' });
  vm.ctx.window.__leko_on_event({
    type: 'state', state: 'speaking', text: 'No go.', duration_ms: 900,
    word_timings_ms: [100, 400], reveal_mode: 'letter',
  });
  assert.equal(vm.els['card-body'].textContent, '');
  const steps = [];
  for (let i = 0; i < 12; i += 1) {
    const next = vm.timers.find((t) => !t.cleared && !t.fired);
    if (!next) { break; }
    vm.advance(next);
    steps.push(vm.els['card-body'].textContent);
  }
  assert.equal(steps[0], 'N');
  assert.equal(steps[1], 'No');
  assert.equal(steps[steps.length - 1], 'No go.');
});
