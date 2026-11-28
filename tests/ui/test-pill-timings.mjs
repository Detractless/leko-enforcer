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
  const makeEl = (id) => {
    const classes = new Set();
    return {
      style: { setProperty() {} },
      classList: {
        add(c) { classes.add(c); },
        remove(c) { classes.delete(c); },
        contains(c) { return classes.has(c); },
      },
      children: [],
      appendChild(c) { this.children.push(c); },
      addEventListener(type, fn) { listeners.set(`${id}:${type}`, fn); },
      contains() { return false; },
      setPointerCapture() {},
      hidden: false,
      textContent: '',
      innerHTML: '',
    };
  };
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
  assert.deepEqual(vm.posted.slice(-2), ['card_dismiss', 'stop']);
  assert.equal(vm.els.pill.className.includes('state-stopped'), true);
  vm.listeners.get('pill:pointerup')({ pointerId: 1 });
  assert.equal(vm.posted.includes('ptt_start'), false);
  assert.equal(vm.posted.includes('ptt_end'), false);
  const stopTimers = vm.timers.filter((t) => !t.cleared && !t.fired);
  assert.equal(stopTimers.length, 1);
  assert.equal(stopTimers[0].ms, 1200);
  vm.advance(stopTimers[0]);
  assert.equal(vm.els.pill.className.includes('state-idle'), true);
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

test('stop_during_thinking_stays_red_even_while_held_and_ignores_stale_idle', () => {
  const vm = mountPillVm();
  vm.ctx.window.__leko_on_event({ type: 'state', state: 'thinking' });
  assert.equal(vm.els.pill.className.includes('state-thinking'), true);
  vm.listeners.get('pill:pointerdown')({ pointerId: 1 });
  assert.deepEqual(vm.posted, ['stop']);
  assert.equal(vm.els.pill.className.includes('state-stopped'), true);
  assert.equal(vm.els.label.textContent, '');
  assert.equal(vm.els.bars.children.filter((b) => b.style.display === 'block').length, 7);
  assert.equal(vm.els.bars.children.every((b) => b.style.height === '2.5px'), true);
  vm.ctx.window.__leko_on_event({ type: 'audio_level', level: 0.95 });
  assert.equal(vm.els.bars.children.every((b) => b.style.height === '2.5px'), true);
  vm.ctx.window.__leko_on_event({ type: 'state', state: 'idle' });
  vm.ctx.window.__leko_on_event({ type: 'state', state: 'speaking' });
  assert.equal(vm.els.pill.className.includes('state-stopped'), true);
  const stopTimers = vm.timers.filter((t) => !t.cleared && !t.fired);
  assert.equal(stopTimers.length, 1);
  vm.advance(stopTimers[0]);
  assert.equal(vm.els.pill.className.includes('state-stopped'), true);
  vm.listeners.get('pill:pointerup')({ pointerId: 1 });
  assert.equal(vm.els.pill.className.includes('state-idle'), true);
  assert.deepEqual(vm.posted, ['stop']);
});

test('card_cursor_updates_close_button_and_card_hover_without_window_focus', () => {
  const vm = mountPillVm();
  vm.els.card.getBoundingClientRect = () => ({ left: 16, top: 12, right: 364, bottom: 200, width: 348, height: 188 });
  vm.els['card-close'].getBoundingClientRect = () => ({ left: 334, top: 20, right: 356, bottom: 42, width: 22, height: 22 });
  vm.ctx.window.__leko_on_event({ type: 'reply_card', text: 'Hello' });
  vm.ctx.window.__leko_on_event({ state: 'card_cursor', x: 345, y: 31 });
  assert.equal(vm.els['card-close'].classList.contains('is-hovered'), true);
  assert.deepEqual(vm.posted, ['card_hover_on']);
  vm.ctx.window.__leko_on_event({ state: 'card_cursor', x: 100, y: 100 });
  assert.equal(vm.els['card-close'].classList.contains('is-hovered'), false);
  assert.deepEqual(vm.posted, ['card_hover_on']);
  vm.ctx.window.__leko_on_event({ state: 'card_cursor', x: -1, y: -1 });
  assert.deepEqual(vm.posted, ['card_hover_on', 'card_hover_off']);
});

test('card_close_click_stops_active_speaking_but_only_dismisses_when_idle', () => {
  const vm = mountPillVm();
  vm.ctx.window.__leko_on_event({ type: 'reply_card', text: 'Alpha beta.' });
  vm.ctx.window.__leko_on_event({
    type: 'state', state: 'speaking', text: 'Alpha beta.', duration_ms: 1500, word_timings_ms: [100, 600],
  });
  vm.listeners.get('card-close:click')({ stopPropagation() {} });
  assert.equal(vm.els.card.hidden, true);
  assert.deepEqual(vm.posted, ['card_dismiss', 'stop']);
  assert.equal(vm.els.pill.className.includes('state-stopped'), true);
  const stopTimers = vm.timers.filter((t) => !t.cleared && !t.fired);
  assert.equal(stopTimers.length, 1);
  assert.equal(stopTimers[0].ms, 1200);
  vm.advance(stopTimers[0]);
  assert.equal(vm.els.pill.className.includes('state-idle'), true);

  vm.posted.length = 0;
  vm.ctx.window.__leko_on_event({ type: 'reply_card', text: 'Quiet note.' });
  vm.listeners.get('card-close:click')({ stopPropagation() {} });
  assert.equal(vm.els.card.hidden, true);
  assert.deepEqual(vm.posted, ['card_dismiss']);
  assert.equal(vm.els.pill.className.includes('state-idle'), true);
});

