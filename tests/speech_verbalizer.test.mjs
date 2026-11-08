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

test('verbalizes_abbreviations_and_durations_for_speech', async () => {
  const script = `
from voice.verbalizer import verbalize_for_speech

# Test e.g., etc., i.e., vs.
t1 = verbalize_for_speech("Block distracting sites, e.g. reddit.com, twitter.com, etc.")
assert "for example" in t1.lower(), f"Expected 'for example' in '{t1}'"
assert "e.g." not in t1.lower()
assert "and more" in t1.lower() or "et cetera" in t1.lower(), f"Expected 'and more' in '{t1}'"
assert "etc" not in t1.lower()

# Test durations
t2 = verbalize_for_speech("Focus session for 25m or 2h vs 45s.")
assert "25 minutes" in t2, f"Expected '25 minutes' in '{t2}'"
assert "2 hours" in t2, f"Expected '2 hours' in '{t2}'"
assert "versus" in t2.lower(), f"Expected 'versus' in '{t2}'"

# Test URLs
t3 = verbalize_for_speech("Visiting https://www.reddit.com/r/all")
assert "https://" not in t3
assert "reddit dot com" in t3.lower()

assert verbalize_for_speech("block/unblock") == "block and or unblock"
assert verbalize_for_speech("and/or") == "and or"
assert verbalize_for_speech("w/o sound") == "without sound"
assert verbalize_for_speech("w/ sound") == "with sound"
assert verbalize_for_speech("1/2 on 10/20/2026") == "1/2 on 10/20/2026"
assert verbalize_for_speech("/Users/me/file") == "/Users/me/file"

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
  assert.equal(code, 0, `Verbalize abbreviations test failed: ${stderr}`);
  assert.ok(stdout.includes('OK'));
});

test('strips_markdown_formatting_and_flattens_tables', async () => {
  const script = `
from voice.verbalizer import verbalize_for_speech

table_md = \"\"\"Here is your current status:
| Target | Type | Status |
| :--- | :--- | :--- |
| reddit.com | Site | Blocked for 2h |
| Discord | App | Allowed |
\"\"\"

v = verbalize_for_speech(table_md)
assert "|" not in v, f"Pipe characters must not be present in spoken text: '{v}'"
assert "---" not in v, f"Table dashes must not be present: '{v}'"
assert "reddit dot com" in v.lower()
assert "discord" in v.lower()

# Test markdown formatting artifacts
formatted = "Action **confirmed** with \\\\\\\\path\\\\\\\\to\\\\\\\\file and \`code_snippet\`."
v2 = verbalize_for_speech(formatted)
assert "**" not in v2
assert "\`" not in v2
assert "\\\\" not in v2

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
  assert.equal(code, 0, `Strip markdown and tables test failed: ${stderr}`);
  assert.ok(stdout.includes('OK'));
});

test('preserves_raw_ui_text_while_verbalizing_audio', async () => {
  const script = `
from voice.verbalizer import verbalize_for_speech
from voice.assistant import process_query
from voice.ui_bridge import UIPipelineBridge

bridge = UIPipelineBridge()
emitted = []
bridge.emit = lambda ev: emitted.append(ev)

spoken_takes = []
def mock_speak(txt, is_cancelled=None):
    spoken_takes.append(txt)

# Simulate process_query with custom mock intent
process_query("status check", "/tmp/settings.zip", bridge=bridge, speak_fn=mock_speak)

# Spoken takes should not contain raw unformatted pipes
for s in spoken_takes:
    assert "|" not in s, f"Spoken take should not contain markdown table pipe: {s}"

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
  assert.equal(code, 0, `Preserve raw UI text test failed: ${stderr}`);
  assert.ok(stdout.includes('OK'));
});

test('review_runner_formats_comparison_and_synthesizes_audio', async () => {
  const script = `
from voice.review_pronunciation import generate_pronunciation_review

samples = [
    "Blocked apps: e.g. Discord, Slack, etc.",
    "| Site | Time |\\n|---|---|\\n| twitter.com | 30m |",
    "Session active for 2h."
]

reviews = generate_pronunciation_review(samples)
assert len(reviews) == 3
for item in reviews:
    assert "original" in item
    assert "verbalized" in item
    assert item["original"] != item["verbalized"]
    assert "|" not in item["verbalized"]

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
  assert.equal(code, 0, `Review runner test failed: ${stderr}`);
  assert.ok(stdout.includes('OK'));
});
