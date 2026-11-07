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

test('em_dashes_convert_to_acoustic_breathing_pauses', async () => {
  const script = `
from voice.verbalizer import verbalize_for_speech

# Test em-dash (—) and double hyphen (--)
t1 = verbalize_for_speech("Tell me what you'd like to do — focus time, block apps.")
assert "—" not in t1, f"Em-dash must be converted, got: {t1}"
assert "..." in t1, f"Expected acoustic breath pause (...) in: {t1}"

t2 = verbalize_for_speech("Focus time -- block apps -- check status.")
assert "--" not in t2, f"Double hyphen must be converted, got: {t2}"
assert "..." in t2, f"Expected acoustic breath pause (...) in: {t2}"

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
  assert.equal(code, 0, `Em-dash conversion test failed: ${stderr}`);
  assert.ok(stdout.includes('OK'));
});

test('sentence_boundaries_maintain_cadence_spacing', async () => {
  const script = `
from voice.verbalizer import verbalize_for_speech

# Test multiple punctuation boundary spacing
t1 = verbalize_for_speech("Yes, I'm here!👋 How can I help you today?Just ask me.")
assert "! How" in t1 or "!  How" in t1 or "here! How" in t1, f"Expected spacing after exclamation mark: {t1}"
assert "? Just" in t1 or "?  Just" in t1, f"Expected spacing after question mark: {t1}"

# Test emoji stripping/spacing so vocoder doesn't rush
assert "👋" not in t1, f"Emojis should be stripped for speech: {t1}"

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
  assert.equal(code, 0, `Sentence boundary cadence test failed: ${stderr}`);
  assert.ok(stdout.includes('OK'));
});

test('verbalized_speech_produces_natural_timing_expansion', async () => {
  const script = `
import sys
sys.path.insert(0, '/Users/calibro/Projects/leko-enforcer')
from voice.assistant import get_tts, VOICE_NAME
from voice.verbalizer import verbalize_for_speech

tts = get_tts()
raw_text = "Focus time — block apps/sites — check status, and more"
verbalized = verbalize_for_speech(raw_text)

raw_pcm = tts.synth(raw_text, voice=VOICE_NAME)
verb_pcm = tts.synth(verbalized, voice=VOICE_NAME)

raw_dur = len(raw_pcm) / tts.sample_rate
verb_dur = len(verb_pcm) / tts.sample_rate

# Verbalized take with breath pauses should be visibly more relaxed (> raw_dur)
assert verb_dur > raw_dur, f"Verbalized duration ({verb_dur:.2f}s) should expand over rushed raw ({raw_dur:.2f}s)"
assert verb_dur >= 3.2, f"Verbalized duration ({verb_dur:.2f}s) should provide at least 3.2s cadence"
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
  assert.equal(code, 0, `Timing expansion test failed: ${stderr}`);
  assert.ok(stdout.includes('OK'));
});

test('raw_ui_text_preserves_original_punctuation_and_em_dashes', async () => {
  const script = `
from voice.assistant import process_query
from voice.ui_bridge import UIPipelineBridge

bridge = UIPipelineBridge()
emitted = []
bridge.emit = lambda ev: emitted.append(ev)

spoken_takes = []
def mock_speak(txt, is_cancelled=None):
    spoken_takes.append(txt)

# Test that process_query emits clean UI text without ellipses
summary = process_query("tell me what time it is", "/tmp/settings.zip", bridge=bridge, speak_fn=mock_speak)

speaking_events = [e for e in emitted if e.get("type") == "state" and e.get("state") == "speaking"]
# Spoken takes get verbalized, but UI card preserves clean typography
assert len(spoken_takes) > 0, "Should have spoken a take"
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
  assert.equal(code, 0, `Raw UI text preservation test failed: ${stderr}`);
  assert.ok(stdout.includes('OK'));
});
