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

test('tool_summaries_are_fluent_sentences_without_artificial_prefixes', async () => {
  const script = `
from voice.assistant import format_status_summary

# Test site status
raw_site_output = "Site reddit.com is closed until 04:00 PM, judged at 02:00 PM in America/New_York. Nothing was saved or changed."
s1 = format_status_summary(raw_site_output, target_type="site", name="reddit.com")
assert not s1.startswith("Status checked:"), f"Should not start with 'Status checked:': {s1}"
assert not s1.startswith("Status:"), f"Should not start with 'Status:': {s1}"
assert not s1.startswith("Site "), f"Should not start with redundant 'Site ': {s1}"
assert "reddit.com is closed until 04:00 PM" in s1, f"Expected clean natural sentence, got: {s1}"

# Test app status
raw_app_output = "App Discord is open for at least the next 30 days, judged at 02:00 PM in America/New_York."
s2 = format_status_summary(raw_app_output, target_type="app", name="Discord")
assert not s2.startswith("App "), f"Should not start with redundant 'App ': {s2}"
assert "Discord is open" in s2, f"Expected clean natural sentence, got: {s2}"

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
  assert.equal(code, 0, `Format status summary test failed: ${stderr}`);
  assert.ok(stdout.includes('OK'));
});

test('model_replies_preserve_natural_intelligence_without_prompt_handcuffs', async () => {
  const script = `
from voice.llm_router import get_system_prompt

sys_prompt = get_system_prompt()
# Prompt should not contain micromanaging negative constraints
assert "never use colons" not in sys_prompt.lower()
assert "always begin with" not in sys_prompt.lower()
assert "do not say" not in sys_prompt.lower()

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
  assert.equal(code, 0, `Model prompt natural intelligence test failed: ${stderr}`);
  assert.ok(stdout.includes('OK'));
});

test('verbalizer_strictly_normalizes_phonetics_without_structural_rewriting', async () => {
  const script = `
from voice.verbalizer import verbalize_for_speech

# Plain natural sentences are preserved verbatim
natural = "I am ready to help manage your focus sessions and distraction blocks."
v1 = verbalize_for_speech(natural)
assert v1 == natural, f"Verbalizer should not alter natural plain text: '{v1}' vs '{natural}'"

# Acoustic phonetics only: expansions and symbol cleaning
mixed = "Focus for 45m with e.g. reddit.com blocked."
v2 = verbalize_for_speech(mixed)
assert "45 minutes" in v2
assert "for example" in v2
assert "reddit dot com" in v2

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
  assert.equal(code, 0, `Verbalizer strictly phonetic test failed: ${stderr}`);
  assert.ok(stdout.includes('OK'));
});

test('e2e_status_and_block_workflows_deliver_clean_spoken_takes', async () => {
  const script = `
from voice.assistant import process_query
from voice.ui_bridge import UIPipelineBridge

bridge = UIPipelineBridge()
emitted = []
bridge.emit = lambda ev: emitted.append(ev)

spoken_takes = []
def mock_speak(txt, is_cancelled=None):
    spoken_takes.append(txt)

# Test query for focus session
summary = process_query("Start a 30m focus session", "/tmp/settings.zip", bridge=bridge, speak_fn=mock_speak)
assert not summary.startswith("Action prepared:"), f"Summary should be natural, got: {summary}"
assert "Starting a focus session of 30m." in summary or "Starting a focus session" in summary

# Spoken take should be cleanly verbalized into 30 minutes
assert any("30 minutes" in s for s in spoken_takes), f"Spoken take should pronounce '30 minutes': {spoken_takes}"

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
  assert.equal(code, 0, `E2E status and block workflows test failed: ${stderr}`);
  assert.ok(stdout.includes('OK'));
});
