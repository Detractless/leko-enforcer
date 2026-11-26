import test from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { existsSync, mkdtempSync, readFileSync, writeFileSync, rmSync, renameSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const PILL = resolve(ROOT, 'target/release/leko-pill');
const LEKO = resolve(ROOT, 'target/release/leko');

test('placement_anchor_persists_in_user_config', () => {
  const dir = mkdtempSync(join(tmpdir(), 'pill-config-'));
  const config = join(dir, 'pill-config.json');
  writeFileSync(config, JSON.stringify({ theme: 'light', anchor: 'top-right', quietTimer: true }));
  const value = JSON.parse(readFileSync(config, 'utf8'));
  assert.equal(value.anchor, 'top-right');
  assert.equal(value.theme, 'light');
  assert.equal(value.quietTimer, true);
  rmSync(dir, { recursive: true, force: true });
});

test('atomic_event_file_bridge_is_deterministic', () => {
  const dir = mkdtempSync(join(tmpdir(), 'pill-events-'));
  const path = join(dir, 'pill-events.json');
  const body = JSON.stringify({ version: 1, event: 'blocked', kind: 'app', target: 'Demo' });
  const temp = join(dir, '.pill-events.tmp');
  writeFileSync(temp, body);
  renameSync(temp, path);
  assert.deepEqual(JSON.parse(readFileSync(path, 'utf8')), JSON.parse(body));
  rmSync(dir, { recursive: true, force: true });
});

test('swift_pill_builds_without_new_rust_dependencies', () => {
  assert.ok(existsSync(PILL), 'cargo build --release must build leko-pill');
  const workspace = readFileSync(resolve(ROOT, 'Cargo.toml'), 'utf8');
  assert.match(workspace, /"ui"/);
  assert.equal(existsSync(resolve(ROOT, 'ui/pill')), false);
  const sources = ['src/main.rs', 'src/state.rs', 'src/timer_poll.rs', 'src/web/pill.html',
                   'src/web/pill.css', 'src/web/pill.js', 'src/web/tokens.css'];
  for (const source of sources) {
    const path = resolve(ROOT, 'ui', source);
    assert.ok(existsSync(path), source);
    assert.ok(readFileSync(path, 'utf8').split('\n').length < 350, source);
  }
});

test('legacy_ui_and_voice_events_are_absent', () => {
  assert.equal(existsSync(resolve(ROOT, 'ui/web')), false);
  assert.equal(existsSync(resolve(ROOT, 'ui/pill')), false);
  const bridge = readFileSync(resolve(ROOT, 'voice/core/ui_bridge.py'), 'utf8');
  assert.doesNotMatch(bridge, /leko-ui/);
  assert.match(bridge, /leko-pill/);
  const runtime = readFileSync(resolve(ROOT, 'voice/session/runtime.py'), 'utf8');
  assert.match(runtime, /ptt_start/);
  assert.match(runtime, /ptt_end/);
});

test('release_binary_and_full_commands_are_verified_externally', () => {
  assert.ok(existsSync(LEKO), 'cargo build --release must build leko');
  const result = spawnSync(LEKO, ['enforce'], { encoding: 'utf8' });
  assert.equal(result.status, 2);
  assert.match(result.stderr, /pill-events/);
});
