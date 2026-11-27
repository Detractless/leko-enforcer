import test from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const PILL = resolve(ROOT, 'target/release/leko-pill');

function headless(events, extraArgs = []) {
  const input = events.map((event) => JSON.stringify(event)).join('\n') + '\n';
  const result = spawnSync(PILL, ['--headless', ...extraArgs], { input, encoding: 'utf8' });
  assert.equal(result.status, 0, result.stderr);
  return result.stdout.trim().split('\n').filter(Boolean).map((line) => JSON.parse(line));
}

function timerProbe(queries) {
  const input = queries.map((q) => JSON.stringify(q)).join('\n') + '\n';
  const result = spawnSync(PILL, ['--timer-probe'], { input, encoding: 'utf8' });
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
    { event: 'thinking', now_ms: 150, duration_ms: 0 },
    { event: 'speaking', now_ms: 200, duration_ms: 0 },
    { event: 'idle', now_ms: 300, duration_ms: 0 },
  ]);
  assert.deepEqual(states.map((state) => state.state), ['held', 'listening', 'thinking', 'speaking', 'idle']);
  assert.deepEqual(states.slice(0, 2).map((state) => state.bars), [7, 7]);
  assert.deepEqual(states.slice(0, 2).map((state) => state.width), [112, 112]);
  assert.equal(states[2].text, '');
  assert.equal(states[2].width, 82);
  assert.equal(states[2].height, 28);
  assert.equal(states[2].bars, 0);
  assert.equal(states[3].text, '');
  assert.equal(states[3].bars, 7);
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
                      'with_decorations(false)', 'with_focused(false)',
                      '.with_html(&html)']) {
    assert.ok(main.includes(flag), flag);
  }
  assert.equal((main.match(/with_transparent\(true\)/g) || []).length, 2);
  assert.ok(existsSync(resolve(ROOT, 'ui/src/web/pill.css')));
});

test('semantic_state_colors_and_thinking_dots_are_configured', () => {
  const tokens = readFileSync(resolve(ROOT, 'ui/src/web/tokens.css'), 'utf8');
  for (const color of ['#181818', '#222222', '#585858', '#F2F2F2', '#8E8E8E',
                       '#3ECF8E', '#F5B747', '#3B82F6',
                       '#FAFAFA', '#FFFFFF', '#CCCCCC', '#141414', '#6E6E6E']) {
    assert.ok(tokens.includes(color), color);
  }
  const css = readFileSync(resolve(ROOT, 'ui/src/web/pill.css'), 'utf8');
  assert.doesNotMatch(css, /chevron|NSMenu/);
  assert.match(css, /\.dots/);
  assert.match(css, /cubic-bezier\(0\.22, 1, 0\.36, 1\)/);
});

test('headless_tests_never_grab_the_microphone_or_open_windows', () => {
  const main = readFileSync(resolve(ROOT, 'ui/src/main.rs'), 'utf8');
  assert.match(main, /--headless/);
  assert.match(main, /event_loop\.run/);
  assert.doesNotMatch(main, /AVAudio|microphone/);
});

test('hover_events_and_audio_levels_drive_pill_visuals', () => {
  const js = readFileSync(resolve(ROOT, 'ui/src/web/pill.js'), 'utf8');
  assert.match(js, /next === 'hover_on'/);
  assert.match(js, /next === 'hover_off'/);
  assert.match(js, /is-hovered/);
  assert.match(js, /event\.type === 'audio_level'/);
  assert.match(js, /WEIGHTS_7/);
  assert.doesNotMatch(js, /MAEVE/);
  const css = readFileSync(resolve(ROOT, 'ui/src/web/pill.css'), 'utf8');
  assert.match(css, /\.leko-pill\.is-hovered \{ background: var\(--pill-hover\); \}/);
  assert.match(css, /\.state-idle\.is-hovered \{ width: 72px; height: 24px; \}/);
  assert.doesNotMatch(css, /:hover/);
});

test('hold_and_click_actions_are_written_with_stdout_flush', () => {
  const js = readFileSync(resolve(ROOT, 'ui/src/web/pill.js'), 'utf8');
  assert.match(js, /pointerdown/);
  assert.match(js, /pointerup/);
  assert.match(js, /pointercancel/);
  assert.match(js, /state !== 'held'/);
  assert.match(js, /state === 'held' && next === 'idle'/);
  assert.match(js, /next === 'blocked' \|\| next === 'error'/);
  assert.match(js, /heldFor <= 200/);
  assert.match(js, /window\.ipc\.postMessage/);
  const main = readFileSync(resolve(ROOT, 'ui/src/main.rs'), 'utf8');
  assert.match(main, /stdout\(\)\.flush/);
});

test('quiet_timer_tracks_active_time_limit_and_returns_idle', () => {
  const [offStart] = headless([{ event: 'timer_start', now_ms: 0, duration_ms: 185000 }]);
  assert.equal(offStart.state, 'idle');
  assert.equal(offStart.dash, true);

  const [onStart, onStop] = headless(
    [
      { event: 'timer_start', now_ms: 0, duration_ms: 185000 },
      { event: 'timer_stop', now_ms: 1000, duration_ms: 0 },
    ],
    ['--quiet-timer'],
  );
  assert.equal(onStart.state, 'timer');
  assert.equal(onStart.text, '03:05');
  assert.equal(onStart.width, 68);
  assert.equal(onStart.height, 24);
  assert.equal(onStop.state, 'idle');
  assert.equal(onStop.timer, '');
  assert.equal(onStop.dash, true);

  const dir = mkdtempSync(join(tmpdir(), 'leko-pill-timer-'));
  try {
    writeFileSync(join(dir, 'pill-events.json'), JSON.stringify({
      version: 1, event: 'tick', usage_timer_seconds: 185, at: '2026-01-01T12:00:00Z',
    }));
    const [defaultOff] = timerProbe([{ dir, now_epoch: 1767268802 }]);
    assert.equal(defaultOff.quiet, false);
    assert.equal(defaultOff.update, 'none');

    writeFileSync(join(dir, 'pill-config.json'), JSON.stringify({ quietTimer: true }));
    const [started, stopped] = timerProbe([
      {
        dir,
        now_epoch: 1767268802,
        events: { version: 1, event: 'tick', usage_timer_seconds: 185, at: '2026-01-01T12:00:00Z' },
      },
      {
        dir,
        now_epoch: 1767268805,
        events: { version: 1, event: 'tick', usage_timer_seconds: null, at: '2026-01-01T12:00:04Z' },
      },
    ]);
    assert.equal(started.quiet, true);
    assert.equal(started.update, 'timer_start');
    assert.equal(started.timer, '03:05');
    assert.equal(stopped.update, 'timer_stop');
    assert.equal(stopped.timer, '');
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }

  const js = readFileSync(resolve(ROOT, 'ui/src/web/pill.js'), 'utf8');
  assert.match(js, /window\.__leko_config\.quietTimer/);
});

test('quiet_timer_respects_15_minute_threshold_and_returns_idle', () => {
  const dir = mkdtempSync(join(tmpdir(), 'leko-pill-threshold-'));
  try {
    writeFileSync(join(dir, 'pill-config.json'), JSON.stringify({ quietTimer: true, timerThresholdMinutes: 15 }));
    const [above, inside, cleared] = timerProbe([
      { dir, now_epoch: 1767268802, events: { version: 1, event: 'tick', usage_timer_seconds: 1200, at: '2026-01-01T12:00:00Z' } },
      { dir, now_epoch: 1767268803, events: { version: 1, event: 'tick', usage_timer_seconds: 600, at: '2026-01-01T12:00:01Z' } },
      { dir, now_epoch: 1767268804, events: { version: 1, event: 'tick', usage_timer_seconds: 1000, at: '2026-01-01T12:00:02Z' } },
    ]);
    assert.equal(above.update, 'none');
    assert.equal(inside.update, 'timer_start');
    assert.equal(inside.timer, '10:00');
    assert.equal(cleared.update, 'timer_stop');
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test('reply_card_streams_text_renders_table_and_handles_hover_and_escape', () => {
  const [opened, speaking, _idle, _hover, stayed, _unhover, escaped] = headless([
    {
      event: 'reply_card',
      now_ms: 0,
      text: 'Upcoming countdowns across active rules.',
      columns: ['Rule', 'Kind', 'Remaining'],
      rows: [['Safari', 'Time Limit', '04:12'], ['Notes', 'Timed Pass', '08:00']],
    },
    { event: 'speaking', now_ms: 100, duration_ms: 2400 },
    { event: 'idle', now_ms: 500 },
    { event: 'card_hover_on', now_ms: 1000 },
    { event: 'check', now_ms: 6000 },
    { event: 'card_hover_off', now_ms: 6100 },
    { event: 'escape', now_ms: 6200 },
  ]);
  assert.equal(opened.card_active, true);
  assert.equal(opened.card_width, 380);
  assert.deepEqual(opened.card_columns, ['Rule', 'Kind', 'Remaining']);
  assert.equal(opened.card_rows.length, 2);
  assert.equal(speaking.state, 'speaking');
  assert.equal(stayed.card_active, true);
  assert.equal(escaped.card_active, false);
  const [fromThinking, forced] = headless([
    { event: 'thinking', now_ms: 0 },
    { event: 'reply_card', now_ms: 50, text: 'Brief reply' },
    { event: 'ptt_start', now_ms: 100 },
    { event: 'force_release', now_ms: 200 },
  ]).filter((_, idx) => idx === 1 || idx === 3);
  assert.equal(fromThinking.state, 'idle');
  assert.equal(fromThinking.card_active, true);
  assert.equal(forced.state, 'idle');
  const [, settled] = headless([
    { event: 'reply_card', now_ms: 0, text: 'Brief reply' },
    { event: 'card_hover_on', now_ms: 100 },
    { event: 'check', now_ms: 5000 },
    { event: 'escape', now_ms: 5100 },
  ]).slice(2);
  assert.equal(settled.card_active, false);
  const js = readFileSync(resolve(ROOT, 'ui/src/web/pill.js'), 'utf8');
  assert.match(js, /streamText/);
  assert.match(js, /tokenizeStream/);
  assert.match(js, /tokenWeight/);
  assert.match(js, /startTokenStream/);
  assert.match(js, /event\.duration_ms/);
  assert.match(js, /event\.word_timings_ms/);
  assert.match(js, /if \(cardActive\) \{ dismissCard\(\); \}/);
  assert.match(js, /renderTable/);
  assert.match(js, /Escape/);
  assert.match(js, /card_hover_on/);
  assert.match(js, /force_release/);
  assert.match(js, /next === 'held' && !localPressed/);
  assert.match(js, /cardClose\.addEventListener\('click'/);
  assert.match(js, /canDismissOutside\(\)/);
  const html = readFileSync(resolve(ROOT, 'ui/src/web/pill.html'), 'utf8');
  assert.match(html, /id="card-close"/);
  const cardCss = readFileSync(resolve(ROOT, 'ui/src/web/card.css'), 'utf8');
  assert.match(cardCss, /width: min\(348px, calc\(100vw - 24px\)\)/);
  assert.match(cardCss, /max-height: calc\(100vh - 72px\)/);
  assert.match(cardCss, /word-wrap: break-word/);
  assert.match(cardCss, /padding: 20px 34px 20px 16px/);
  assert.match(cardCss, /\.card-close/);
  assert.match(cardCss, /\.card-close:hover/);
  assert.match(cardCss, /--pill-close-border/);
  assert.match(cardCss, /--pill-close-hover-bg/);
  const mainRs = readFileSync(resolve(ROOT, 'ui/src/main.rs'), 'utf8');
  assert.match(mainRs, /webview\.set_bounds/);
  assert.match(mainRs, /poll_outside_click/);
});

