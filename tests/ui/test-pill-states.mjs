import test from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { existsSync, readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const PILL = resolve(ROOT, 'target/release/leko-pill');

function headless(events) {
  const input = events.map((event) => JSON.stringify(event)).join('\n') + '\n';
  const result = spawnSync(PILL, ['--headless'], { input, encoding: 'utf8' });
  assert.equal(result.status, 0, result.stderr);
  return result.stdout.trim().split('\n').filter(Boolean).map((line) => JSON.parse(line));
}

test('one_unified_pill_replaces_the_circle_ui', () => {
  assert.equal(existsSync(resolve(ROOT, 'ui/web')), false);
  assert.equal(existsSync(resolve(ROOT, 'ui/src')), true);
  assert.equal(existsSync(resolve(ROOT, 'ui/pill')), false);
  assert.ok(existsSync(resolve(ROOT, 'ui/src/web/pill.html')));
});

test('idle_pill_is_a_tiny_textless_capsule', () => {
  const [idle] = headless([{ event: 'idle', now_ms: 0, duration_ms: 0 }]);
  assert.equal(idle.state, 'idle');
  assert.equal(idle.text, '');
  assert.equal(idle.width, 54);
  assert.equal(idle.height, 16);
  assert.equal(idle.dash, true);
  assert.equal(idle.bars, 0);
});

test('blocked_flash_widens_then_shrinks', () => {
  const [flash, settled] = headless([
    { event: 'blocked', now_ms: 0, duration_ms: 0 },
    { event: 'check', now_ms: 3000, duration_ms: 0 },
  ]);
  assert.equal(flash.state, 'blocked');
  assert.equal(flash.text, 'Blocked');
  assert.equal(flash.width, 92);
  assert.equal(flash.height, 28);
  assert.equal(settled.state, 'idle');
  assert.equal(settled.width, 54);
});

test('dictation_states_flow_from_hold_to_idle', () => {
  const states = headless([
    { event: 'ptt_start', now_ms: 0, duration_ms: 0 },
    { event: 'ptt_end', now_ms: 100, duration_ms: 0 },
    { event: 'speaking', now_ms: 200, duration_ms: 0 },
    { event: 'idle', now_ms: 300, duration_ms: 0 },
  ]);
  assert.deepEqual(states.map((state) => state.state), ['held', 'listening', 'speaking', 'idle']);
  assert.deepEqual(states.slice(0, 2).map((state) => state.bars), [5, 5]);
  assert.deepEqual(states.slice(0, 2).map((state) => state.width), [112, 112]);
  assert.equal(states[2].text, 'MAEVE');
  assert.equal(states[2].bars, 3);
});

test('quiet_timer_only_appears_for_an_active_timed_pass', () => {
  const [started, cleared] = headless([
    { event: 'timer_start', now_ms: 0, duration_ms: 252000 },
    { event: 'timer_stop', now_ms: 1000, duration_ms: 0 },
  ]);
  assert.equal(started.timer, '04:12');
  assert.equal(cleared.timer, '');
});

test('panel_configuration_is_non_activating_and_always_on_top', () => {
  const main = readFileSync(resolve(ROOT, 'ui/src/main.rs'), 'utf8');
  for (const flag of ['with_transparent(true)', 'with_always_on_top(true)',
                      'with_decorations(false)', 'with_focused(false)']) {
    assert.ok(main.includes(flag), flag);
  }
  assert.ok(existsSync(resolve(ROOT, 'ui/src/web/pill.css')));
});

test('both_themes_are_monochrome_without_dots_or_chevrons', () => {
  const tokens = readFileSync(resolve(ROOT, 'ui/src/web/tokens.css'), 'utf8');
  for (const color of ['#181818', '#222222', '#303030', '#F2F2F2', '#8E8E8E',
                       '#FAFAFA', '#FFFFFF', '#CCCCCC', '#141414', '#6E6E6E']) {
    assert.ok(tokens.includes(color), color);
  }
  assert.doesNotMatch(tokens, /--color-(accent|blue|red|green|pink|orange)/);
  const css = readFileSync(resolve(ROOT, 'ui/src/web/pill.css'), 'utf8');
  assert.doesNotMatch(css, /chevron|dot|NSMenu/);
  assert.match(css, /cubic-bezier\(0\.22, 1, 0\.36, 1\)/);
});

test('headless_tests_never_grab_the_microphone_or_open_windows', () => {
  const main = readFileSync(resolve(ROOT, 'ui/src/main.rs'), 'utf8');
  assert.match(main, /--headless/);
  assert.match(main, /event_loop\.run/);
  assert.doesNotMatch(main, /AVAudio|microphone/);
});

test('hold_and_click_actions_are_written_with_stdout_flush', () => {
  const js = readFileSync(resolve(ROOT, 'ui/src/web/pill.js'), 'utf8');
  assert.match(js, /pointerdown/);
  assert.match(js, /pointerup/);
  assert.match(js, /heldFor <= 200/);
  assert.match(js, /window\.ipc\.postMessage/);
  const main = readFileSync(resolve(ROOT, 'ui/src/main.rs'), 'utf8');
  assert.match(main, /stdout\(\)\.flush/);
});
