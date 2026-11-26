import test from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const PILL = resolve(ROOT, 'target/release/leko-pill');

function probe(points) {
  const input = points.map((point) => JSON.stringify(point)).join('\n') + '\n';
  const result = spawnSync(PILL, ['--input-probe'], { input, encoding: 'utf8' });
  assert.equal(result.status, 0, result.stderr);
  return result.stdout.trim().split('\n').filter(Boolean).map((line) => JSON.parse(line));
}

const inside = { x: 150, y: 120 };
const outside = { x: 500, y: 500 };

test('cursor_poll_uses_an_injectable_provider', () => {
  const source = readFileSync(resolve(ROOT, 'ui/src/native_input.rs'), 'utf8');
  assert.match(source, /trait CursorProvider/);
  assert.match(source, /struct TestCursor/);
  assert.match(source, /NSEvent::mouseLocation/);
  const [first] = probe([inside]);
  assert.equal(first.inside, true);
});

test('cursor_inside_and_outside_emit_hover_transitions', () => {
  const states = probe([inside, inside, outside, outside, inside]);
  assert.deepEqual(states.map((item) => item.event), ['enter', 'none', 'leave', 'none', 'enter']);
});

test('poll_cadence_adapts_to_proximity', () => {
  const [near, far] = probe([inside, outside]);
  assert.equal(near.cadence_ms, 16);
  assert.equal(far.cadence_ms, 90);
  const source = readFileSync(resolve(ROOT, 'ui/src/native_input.rs'), 'utf8');
  assert.match(source, /if near \{ 16 \} else \{ 90 \}/);
});

test('click_through_flips_only_while_inside', () => {
  const [near, far] = probe([inside, outside]);
  assert.equal(near.ignore_mouse, false);
  assert.equal(far.ignore_mouse, true);
  const source = readFileSync(resolve(ROOT, 'ui/src/native_input.rs'), 'utf8');
  assert.match(source, /setIgnoresMouseEvents/);
});

test('first_mouse_down_is_accepted_without_activation', () => {
  const main = readFileSync(resolve(ROOT, 'ui/src/main.rs'), 'utf8');
  assert.match(main, /with_focused\(false\)/);
  assert.match(main, /set_ignores_mouse/);
  const js = readFileSync(resolve(ROOT, 'ui/src/web/pill.js'), 'utf8');
  assert.match(js, /pointerdown/);
});

test('interaction_never_steals_keyboard_focus', () => {
  const main = readFileSync(resolve(ROOT, 'ui/src/main.rs'), 'utf8');
  assert.match(main, /focused\(false\)/);
  assert.doesNotMatch(main, /activateIgnoringOtherApps/);
});

test('hold_and_click_still_emit_the_same_actions', () => {
  const js = readFileSync(resolve(ROOT, 'ui/src/web/pill.js'), 'utf8');
  assert.match(js, /pointerup/);
  assert.match(js, /heldFor <= 200/);
  assert.match(js, /post\('ptt_start'\)/);
  assert.match(js, /post\('ptt_end'\)/);
});

test('platform_hooks_keep_non_macos_builds_safe', () => {
  const source = readFileSync(resolve(ROOT, 'ui/src/native_input.rs'), 'utf8');
  assert.match(source, /#\[cfg\(not\(target_os = "macos"\)\)\]/);
  assert.match(source, /fn set_ignores_mouse/);
});

test('headless_input_tests_are_deterministic', () => {
  const main = readFileSync(resolve(ROOT, 'ui/src/main.rs'), 'utf8');
  assert.match(main, /--input-probe/);
  assert.match(main, /--headless/);
  assert.doesNotMatch(main, /AVAudio|microphone/);
  const a = probe([inside, outside]);
  const b = probe([inside, outside]);
  assert.deepEqual(a, b);
});

test('release_binary_and_full_commands_are_verified_externally', () => {
  assert.ok(PILL);
  const result = spawnSync(PILL, ['--headless'], {
    input: JSON.stringify({ event: 'idle', now_ms: 0, duration_ms: 0 }) + '\n',
    encoding: 'utf8',
  });
  assert.equal(result.status, 0, result.stderr);
  assert.equal(JSON.parse(result.stdout.trim()).state, 'idle');
});
