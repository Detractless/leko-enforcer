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
  assert.equal(existsSync(resolve(ROOT, 'ui/src')), false);
  assert.ok(existsSync(resolve(ROOT, 'ui/pill/main.swift')));
});

test('idle_pill_is_a_tiny_textless_capsule', () => {
  const [idle] = headless([{ event: 'idle', now_ms: 0, duration_ms: 0 }]);
  assert.equal(idle.state, 'idle');
  assert.equal(idle.text, '');
  assert.equal(idle.width, 54);
});

test('blocked_flash_widens_then_shrinks', () => {
  const [flash, settled] = headless([
    { event: 'blocked', now_ms: 0, duration_ms: 0 },
    { event: 'check', now_ms: 3000, duration_ms: 0 },
  ]);
  assert.equal(flash.state, 'blocked');
  assert.equal(flash.text, 'Blocked');
  assert.equal(flash.width, 90);
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
});

test('quiet_timer_only_appears_for_an_active_timed_pass', () => {
  const [started, cleared] = headless([
    { event: 'timer_start', now_ms: 0, duration_ms: 60000 },
    { event: 'timer_stop', now_ms: 1000, duration_ms: 0 },
  ]);
  assert.equal(started.timer, '01:00');
  assert.equal(cleared.timer, '');
});

test('panel_configuration_is_non_activating_and_always_on_top', () => {
  const source = readFileSync(resolve(ROOT, 'ui/pill/PillPanel.swift'), 'utf8');
  assert.match(source, /nonactivatingPanel/);
  assert.match(source, /level = \.floating/);
  const main = readFileSync(resolve(ROOT, 'ui/pill/main.swift'), 'utf8');
  assert.match(main, /setActivationPolicy\(\.accessory\)/);
});

test('both_themes_are_monochrome_without_dots_or_chevrons', () => {
  const source = readFileSync(resolve(ROOT, 'ui/pill/PillPanel.swift'), 'utf8');
  assert.doesNotMatch(source, /NSColor\.system/);
  assert.doesNotMatch(source, /chevron|disclosureTriangle|NSMenu/);
  assert.match(source, /NSColor\.white/);
  assert.match(source, /NSColor\.black/);
});

test('headless_tests_never_grab_the_microphone_or_open_windows', () => {
  const main = readFileSync(resolve(ROOT, 'ui/pill/main.swift'), 'utf8');
  assert.match(main, /--headless/);
  assert.match(main, /NSApplication\.shared\.run\(\)/);
  const panel = readFileSync(resolve(ROOT, 'ui/pill/PillPanel.swift'), 'utf8');
  assert.doesNotMatch(panel, /AVAudio|AVAudioSession|microphone/);
});
