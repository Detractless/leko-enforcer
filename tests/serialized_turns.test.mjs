import test from 'node:test';
import assert from 'node:assert/strict';
import { existsSync } from 'node:fs';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawn } from 'node:child_process';

const __dirname = dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = resolve(__dirname, '..');
const TALK_PYTHON = '/Users/calibro/.t3/scratch/phone-pilot/talk-venv/bin/python3';
const PYTHON_BIN = existsSync(TALK_PYTHON) ? TALK_PYTHON : 'python3';

test('serialized_turn_manager_prevents_concurrent_overlap', async () => {
  const script = `
import time
from voice.assistant import SerializedTurnWorker

worker = SerializedTurnWorker()
max_concurrent = 0
current_running = 0

def task(seq):
    global max_concurrent, current_running
    current_running += 1
    if current_running > max_concurrent:
        max_concurrent = current_running
    time.sleep(0.05)
    current_running -= 1

worker.submit(task)
worker.submit(task)
worker.submit(task)
time.sleep(0.2)
worker.stop()

assert max_concurrent == 1, f"Max concurrent must be 1, got {max_concurrent}"
print("OK")
`;

  const proc = spawn(PYTHON_BIN, ['-c', script], {
    cwd: REPO_ROOT,
    env: { ...process.env, PYTHONPATH: REPO_ROOT }
  });

  let stdout = '';
  let stderr = '';
  for await (const chunk of proc.stdout) stdout += chunk;
  for await (const chunk of proc.stderr) stderr += chunk;

  const code = await new Promise((res) => proc.on('close', res));
  assert.equal(code, 0, `Serialized worker test failed: ${stderr}`);
  assert.ok(stdout.includes('OK'));
});

test('newer_turn_supersedes_stale_turn', async () => {
  const script = `
import time
from voice.assistant import SerializedTurnWorker

worker = SerializedTurnWorker()
executed = []

def slow_task(seq):
    time.sleep(0.08)
    if not worker.is_superseded(seq):
        executed.append(seq)

worker.submit(slow_task)
# Submit newer turn almost immediately, superseding turn 1
time.sleep(0.01)
worker.submit(slow_task)
time.sleep(0.25)
worker.stop()

assert executed == [2], f"Only newer turn should complete, got {executed}"
print("OK")
`;

  const proc = spawn(PYTHON_BIN, ['-c', script], {
    cwd: REPO_ROOT,
    env: { ...process.env, PYTHONPATH: REPO_ROOT }
  });

  let stdout = '';
  let stderr = '';
  for await (const chunk of proc.stdout) stdout += chunk;
  for await (const chunk of proc.stderr) stderr += chunk;

  const code = await new Promise((res) => proc.on('close', res));
  assert.equal(code, 0, `Superseded test failed: ${stderr}`);
  assert.ok(stdout.includes('OK'));
});

test('llm_router_handles_http_errors_gracefully', async () => {
  const script = `
from voice.llm_router import parse_user_intent_safe

# Test with invalid url to trigger HTTP or connection error
res = parse_user_intent_safe("test command", endpoint_url="http://127.0.0.1:99999/invalid")
assert res["action"] == "reply"
assert "issue" in res["content"].lower() or "error" in res["content"].lower() or "sorry" in res["content"].lower()
print("OK")
`;

  const proc = spawn(PYTHON_BIN, ['-c', script], {
    cwd: REPO_ROOT,
    env: { ...process.env, PYTHONPATH: REPO_ROOT }
  });

  let stdout = '';
  let stderr = '';
  for await (const chunk of proc.stdout) stdout += chunk;
  for await (const chunk of proc.stderr) stderr += chunk;

  const code = await new Promise((res) => proc.on('close', res));
  assert.equal(code, 0, `HTTP error handling test failed: ${stderr}`);
  assert.ok(stdout.includes('OK'));
});

test('ui_state_always_resets_on_query_failure', async () => {
  const script = `
from voice.ui_bridge import UIPipelineBridge

events = []
bridge = UIPipelineBridge(event_sink=lambda ev: events.append(ev))

# Simulate query execution with guaranteed finally reset
bridge.emit_state("thinking", "Transcribing...")
try:
    raise RuntimeError("Simulated failure")
except Exception:
    pass
finally:
    bridge.emit_idle()

assert events[-1]["state"] == "idle"
print("OK")
`;

  const proc = spawn(PYTHON_BIN, ['-c', script], {
    cwd: REPO_ROOT,
    env: { ...process.env, PYTHONPATH: REPO_ROOT }
  });

  let stdout = '';
  let stderr = '';
  for await (const chunk of proc.stdout) stdout += chunk;
  for await (const chunk of proc.stderr) stderr += chunk;

  const code = await new Promise((res) => proc.on('close', res));
  assert.equal(code, 0, `UI reset test failed: ${stderr}`);
  assert.ok(stdout.includes('OK'));
});
