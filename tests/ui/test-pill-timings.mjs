import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { runInNewContext } from 'node:vm';

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '../..');

export function mountPillVm() {
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
        toggle(c, on) { if (on) classes.add(c); else classes.delete(c); },
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
    ['pill', 'bars', 'label', 'card', 'card-close', 'card-user', 'card-query', 'card-body', 'card-table', 'card-head', 'card-rows']
      .map((k) => [k, makeEl(k)]),
  );
  let now = 1000;
  const timers = [];
  const posted = [];
  const ctx = {
    document: {
      getElementById: (id) => els[id] || makeEl(id),
      createElement: () => makeEl('el'),
      addEventListener(type, fn) { const key = `document:${type}`, prev = listeners.get(key); listeners.set(key, prev ? (event) => { prev(event); fn(event); } : fn); },
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

test('yellow_transcript_preview_and_simultaneous_reveal_flow', () => {
  const vm = mountPillVm();
  vm.els.card.hidden = true;
  vm.listeners.get('pill:pointerdown')({ pointerId: 1 });
  vm.ctx.performance.now = () => 1350;
  vm.listeners.get('pill:pointerup')({ pointerId: 1 });
  assert.equal(vm.els.pill.className.includes('state-listening'), true);
  assert.equal(vm.els.card.hidden, true);
  vm.ctx.window.__leko_on_event({ type: 'state', state: 'thinking', query: 'Pause youtube' });
  assert.equal(vm.els.pill.className.includes('state-thinking'), true);
  assert.equal(vm.els.card.hidden, false);
  assert.equal(vm.els.card.classList.contains('is-thinking'), true);
  assert.equal(vm.els['card-query'].textContent, 'Pause youtube');
  assert.equal(vm.els['card-body'].textContent, 'Thinking...');

  // Clicking X during Yellow thinking cancels speaking before TTS starts
  vm.listeners.get('card-close:click')({ stopPropagation() {} });
  assert.equal(vm.els.card.hidden, true);
  assert.deepEqual(vm.posted.slice(-2), ['card_dismiss', 'stop']);
  assert.equal(vm.els.pill.className.includes('state-stopped'), true);
});


test('right_click_does_not_start_or_interrupt_voice', () => {
  const vm = mountPillVm();
  vm.listeners.get('pill:pointerdown')({ pointerId: 1, button: 2 });
  assert.deepEqual(vm.posted, []);
  vm.ctx.window.__leko_on_event({ state: 'speaking' });
  vm.listeners.get('pill:pointerdown')({ pointerId: 2, button: 2 });
  assert.deepEqual(vm.posted, []);
  assert.equal(vm.els.pill.className.includes('state-speaking'), true);
});

test('move_mode_drag_does_not_trigger_voice', () => {
  const vm = mountPillVm();
  vm.listeners.get('pill:contextmenu')({ preventDefault() {} });
  vm.listeners.get('pill:pointerdown')({ pointerId: 1, button: 0 });
  vm.listeners.get('pill:pointerup')({ pointerId: 1 });
  vm.listeners.get('pill:contextmenu')({ preventDefault() {} });
  assert.deepEqual(vm.posted, ['move_toggle', 'move_start', 'move_end', 'move_toggle']);
});

test('right_release_preserves_held_voice', () => {
  const vm = mountPillVm();
  vm.listeners.get('pill:pointerdown')({ pointerId: 1, button: 0 });
  vm.listeners.get('pill:pointerup')({ pointerId: 2, button: 2 });
  assert.deepEqual(vm.posted, ['ptt_start']);
  assert.equal(vm.els.pill.className.includes('state-held'), true);
});

test('move_mode_opens_during_quiet_timer', () => {
  const vm = mountPillVm();
  vm.ctx.window.__leko_config.quietTimer = true;
  vm.ctx.window.__leko_on_event({ state: 'timer_start', timer: '01:00' });
  vm.listeners.get('pill:contextmenu')({ preventDefault() {} });
  assert.deepEqual(vm.posted, ['move_toggle']);
});

test('keyboard_move_routes_arrows_and_cancel_without_voice', () => {
  const vm = mountPillVm();
  vm.ctx.window.__leko_keyboard_move();
  for (const key of ['ArrowLeft', 'Escape']) vm.listeners.get('document:keydown')({ key, preventDefault() {} });
  assert.deepEqual(vm.posted, ['move_keyboard_start', 'move_key_left', 'move_key_cancel']);
});


test('spoken_pill_stays_still_until_reply_card_closes', () => {
  const vm = mountPillVm();
  vm.ctx.window.__leko_on_event({ state: 'reply_card', text: '' });
  vm.ctx.window.__leko_on_event({ state: 'speaking' });
  vm.ctx.window.__leko_on_event({ state: 'idle' });
  assert.equal(vm.els.pill.className.includes('state-speaking'), true);
  assert.equal(vm.els.card.hidden, false);
  assert.ok(vm.els.bars.children.every((bar) => bar.style.height === '2.5px'));
  const collapse = vm.timers.find((t) => t.ms === 3500 && !t.cleared);
  assert.ok(collapse);
  vm.advance(collapse);
  assert.equal(vm.els.card.style.opacity, '0');
  assert.equal(vm.els.pill.className.includes('state-idle'), true);
  vm.advance(vm.timers.find((t) => t.ms === 300 && !t.cleared && !t.fired));
  assert.equal(vm.els.card.hidden, true);
});

test('spoken_pill_shrinks_when_reply_card_is_closed_manually', () => {
  const vm = mountPillVm();
  vm.ctx.window.__leko_on_event({ state: 'reply_card', text: '' });
  vm.ctx.window.__leko_on_event({ state: 'speaking' });
  vm.ctx.window.__leko_on_event({ state: 'idle' });
  vm.listeners.get('card-close:click')({ stopPropagation() {} });
  assert.equal(vm.els.card.style.opacity, '0');
  assert.equal(vm.els.pill.className.includes('state-idle'), true);
  vm.advance(vm.timers.find((t) => t.ms === 300 && !t.cleared && !t.fired));
  assert.equal(vm.els.card.hidden, true);
});

test('new_reply_cancels_pending_card_close', () => {
  const vm = mountPillVm();
  vm.ctx.window.__leko_on_event({ state: 'reply_card', text: '' });
  vm.ctx.window.__leko_on_event({ state: 'speaking' });
  vm.ctx.window.__leko_on_event({ state: 'idle' });
  vm.listeners.get('card-close:click')({ stopPropagation() {} });
  const close = vm.timers.find((t) => t.ms === 300 && !t.cleared && !t.fired);
  vm.ctx.window.__leko_on_event({ state: 'reply_card', text: 'Next reply' });
  assert.equal(close.cleared, true);
  assert.equal(vm.els.card.hidden, false);
  assert.equal(vm.els.card.style.opacity, '');
});
