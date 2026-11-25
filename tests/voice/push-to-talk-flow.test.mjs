import test from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { existsSync, readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const PILL = resolve(ROOT, 'target/release/leko-pill');

function headless(events) {
  const input = events.map((event) => JSON.stringify(event)).join('\n') + '\n';
  const result = spawnSync(PILL, ['--headless'], { input, encoding: 'utf8' });
  assert.equal(result.status, 0, result.stderr);
  return result.stdout.trim().split('\n').filter(Boolean).map((line) => JSON.parse(line));
}

test('pill_hold_and_release_match_existing_voice_actions', () => {
  const states = headless([
    { event: 'ptt_start', now_ms: 0, duration_ms: 0 },
    { event: 'ptt_end', now_ms: 100, duration_ms: 0 },
  ]);
  assert.deepEqual(states.map((state) => state.state), ['held', 'listening']);
  const bridge = readFileSync(resolve(ROOT, 'voice/core/ui_bridge.py'), 'utf8');
  assert.match(bridge, /ptt_start/);
  assert.match(bridge, /ptt_end/);
});

test('pill_states_keep_core_voice_names_without_circle_ui', () => {
  const states = headless([
    { event: 'idle', now_ms: 0, duration_ms: 0 },
    { event: 'thinking', now_ms: 0, duration_ms: 0 },
    { event: 'speaking', now_ms: 0, duration_ms: 0 },
    { event: 'error', now_ms: 0, duration_ms: 0 },
  ]);
  assert.deepEqual(states.map((state) => state.state), ['idle', 'thinking', 'speaking', 'error']);
  assert.equal(existsSync(resolve(ROOT, 'ui/web')), false);
});

test('pill_has_no_visible_text_and_no_circle_controls', () => {
  const source = readFileSync(resolve(ROOT, 'ui/pill/PillPanel.swift'), 'utf8');
  assert.doesNotMatch(source, /system(Pink|Blue|Red|Green|Orange|Yellow|Purple|Teal|Indigo)/);
  assert.doesNotMatch(source, /chevron|disclosureTriangle|NSMenu/);
});
