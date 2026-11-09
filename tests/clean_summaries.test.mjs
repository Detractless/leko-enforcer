import test from 'node:test';
import assert from 'node:assert/strict';
import { VOICE_BRIDGE_PRELUDE } from './voice-script-fixtures.mjs';
import { runPython } from './python-runner.mjs';

async function assertPythonTest(script, label) {
  const { code, stdout, stderr } = await runPython(script);
  assert.equal(code, 0, `${label} failed: ${stderr}`);
  assert.ok(stdout.includes('OK'));
}

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

# Test a device status: the redundant prefix goes, the action stays.
raw_device_output = "Device: the screen is locked until 04:00 PM, judged at 02:00 PM in America/New_York. Closed by block \\"freeze\\" rule \\"always\\" (always). Nothing was saved or changed."
s3 = format_status_summary(raw_device_output, target_type="device")
assert not s3.startswith("Device"), f"Should not start with redundant 'Device': {s3}"
assert s3 == "the screen is locked until 04:00 PM.", f"Expected clean device sentence, got: {s3}"

print("OK")
`;

  await assertPythonTest(script, 'Format status summary test');
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

  await assertPythonTest(script, 'Model prompt natural intelligence test');
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

  await assertPythonTest(script, 'Verbalizer strictly phonetic test');
});

test('e2e_status_and_block_workflows_deliver_clean_spoken_takes', async () => {
  const script = `
import voice.assistant as assistant
${VOICE_BRIDGE_PRELUDE}
# Mock parse_user_intent_safe to avoid flaky HTTP 500 when llama-server is bombarded concurrently
orig_parse = assistant.parse_user_intent_safe
assistant.parse_user_intent_safe = lambda q, **kwargs: {"action": "tool_call", "tool": "start_session", "arguments": {"length": "30m"}}

try:
    summary = assistant.process_query("Start a 30m focus session", "/tmp/settings.zip", bridge=bridge, speak_fn=mock_speak)
    assert not summary.startswith("Action prepared:"), f"Summary should be natural, got: {summary}"
    assert "Prepared a focus session of 30m." in summary
    assert any("30 minutes" in s for s in spoken_takes), f"Spoken take should pronounce '30 minutes': {spoken_takes}"
    print("OK")
finally:
    assistant.parse_user_intent_safe = orig_parse
`;

  await assertPythonTest(script, 'E2E status and block workflows test');
});
