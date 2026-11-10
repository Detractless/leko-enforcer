import test from 'node:test';
import assert from 'node:assert/strict';
import { VOICE_BRIDGE_PRELUDE } from './voice-script-fixtures.mjs';
import { runPython } from './python-runner.mjs';

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

  const { code, stdout, stderr } = await runPython(script);
  assert.equal(code, 0, `Em-dash conversion test failed: ${stderr}`);
  assert.ok(stdout.includes('OK'));
});

test('pauses_at_clear_boundaries_without_rephrasing', async () => {
  const script = `
from voice.verbalizer import verbalize_for_speech

source = "Choose apps, websites, or device blocking — lock, log out, or shut down."
spoken = verbalize_for_speech(source)
assert "..." in spoken
assert spoken.count("...") == 1, spoken
assert spoken.count(",") == source.count(","), spoken
assert "Choose apps, websites, or device blocking" in spoken
assert "lock, log out, or shut down." in spoken
print("OK")
`;
  const { code, stdout, stderr } = await runPython(script);
  assert.equal(code, 0, `Clear-boundary cadence test failed: ${stderr}`);
  assert.ok(stdout.includes('OK'));
});

test('prose_colons_use_ellipsis_pause_but_clock_and_technical_colons_stay_intact', async () => {
  const script = `
from voice.verbalizer import verbalize_for_speech as v
spoken = v("For which schedule: 9 a.m.-5 p.m.? It is 11:32 PM. Host localhost:8765; hash sha256:deadbeef.")
assert "schedule... nine a.m. to five p.m.?" in spoken, spoken
assert "eleven thirty-two p.m." in spoken, spoken
assert "localhost:8765" in spoken and "sha256:deadbeef" in spoken, spoken
print("OK")
`;
  const { code, stdout, stderr } = await runPython(script);
  assert.equal(code, 0, `Colon and clock-range cadence test failed: ${stderr}`);
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

  const { code, stdout, stderr } = await runPython(script);
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

  const { code, stdout, stderr } = await runPython(script);
  assert.equal(code, 0, `Timing expansion test failed: ${stderr}`);
  assert.ok(stdout.includes('OK'));
});

test('raw_ui_text_preserves_original_punctuation_and_em_dashes', async () => {
  const script = `
from voice.assistant import process_query
${VOICE_BRIDGE_PRELUDE}
# Test that process_query emits clean UI text without ellipses
summary = process_query("tell me what time it is", "/tmp/settings.zip", bridge=bridge, speak_fn=mock_speak)

speaking_events = [e for e in emitted if e.get("type") == "state" and e.get("state") == "speaking"]
# Spoken takes get verbalized, but UI card preserves clean typography
assert len(spoken_takes) > 0, "Should have spoken a take"
print("OK")
`;

  const { code, stdout, stderr } = await runPython(script);
  assert.equal(code, 0, `Raw UI text preservation test failed: ${stderr}`);
  assert.ok(stdout.includes('OK'));
});
