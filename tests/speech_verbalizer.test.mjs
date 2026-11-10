import test from 'node:test';
import assert from 'node:assert/strict';
import { VOICE_BRIDGE_PRELUDE } from './voice-script-fixtures.mjs';
import { runPython } from './python-runner.mjs';

async function assertPython(script, label) {
  const { code, stdout, stderr } = await runPython(script);
  assert.equal(code, 0, `${label} failed: ${stderr}`);
  assert.ok(stdout.includes('OK'));
}

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
assert "twenty-five minutes" in t2, f"Expected 'twenty-five minutes' in '{t2}'"
assert "two hours" in t2, f"Expected 'two hours' in '{t2}'"
assert "versus" in t2.lower(), f"Expected 'versus' in '{t2}'"

# Test URLs
t3 = verbalize_for_speech("Visiting https://www.reddit.com/r/all")
assert t3 == "Visiting https://www.reddit.com/r/all"

assert verbalize_for_speech("block/unblock") == "block or unblock"
assert verbalize_for_speech("and/or") == "and or"
assert verbalize_for_speech("w/o sound") == "without sound"
assert verbalize_for_speech("w/ sound") == "with sound"
assert verbalize_for_speech("1/2 on 10/20/2026") == "one half on October twentieth, twenty twenty-six"
assert verbalize_for_speech("/Users/me/file") == "/Users/me/file"

print("OK")
`;

  const { code, stdout, stderr } = await runPython(script);
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
assert "code_snippet" in v2

print("OK")
`;

  const { code, stdout, stderr } = await runPython(script);
  assert.equal(code, 0, `Strip markdown and tables test failed: ${stderr}`);
  assert.ok(stdout.includes('OK'));
});

test('preserves_raw_ui_text_while_verbalizing_audio', async () => {
  const script = `
from voice.verbalizer import verbalize_for_speech
from voice.assistant import process_query
${VOICE_BRIDGE_PRELUDE}
# Simulate process_query with custom mock intent
process_query("status check", "/tmp/settings.zip", bridge=bridge, speak_fn=mock_speak)

# Spoken takes should not contain raw unformatted pipes
for s in spoken_takes:
    assert "|" not in s, f"Spoken take should not contain markdown table pipe: {s}"

print("OK")
`;

  const { code, stdout, stderr } = await runPython(script);
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

  const { code, stdout, stderr } = await runPython(script);
  assert.equal(code, 0, `Review runner test failed: ${stderr}`);
  assert.ok(stdout.includes('OK'));
});

test('keeps_non_speech_text_unchanged', async () => assertPython(`
from voice import assistant
from voice.session.query_flow import _respond
from voice.session.conversation import Conversation
from voice.verbalizer import verbalize_for_speech

raw = "The speed is 5 km/h at 11:32 PM."
class Bridge:
    def emit_speaking(self, text): self.text = text
    def emit_idle(self): pass
bridge, state, spoken = Bridge(), Conversation(), []
result = _respond("when", state, bridge, lambda text, cancel=None: spoken.append(text), lambda: False, raw)
assert result == raw and bridge.text == raw
assert state.history[-1]["content"] == raw
assert spoken == [verbalize_for_speech(raw)] and spoken[0] != raw
print("OK")
`, 'TTS-only boundary'));

test('preserves_urls_paths_code_and_identifiers', async () => assertPython(`
from voice.verbalizer import verbalize_for_speech
raw = "https://example.dev/a?x=1; sam@example.dev; /Users/mei/a.py; src/voice/assistant.py; C:\\\\Users\\\\mei\\\\a.py; \`user_id 3.14\`; v1.2.3; REQ-123; sha256:deadbeef; 12:34:56; 2026-11-10T12:34:56Z"
spoken = verbalize_for_speech(raw)
for literal in ["https://example.dev/a?x=1", "sam@example.dev", "/Users/mei/a.py", "src/voice/assistant.py", "C:\\\\Users\\\\mei\\\\a.py",
                "\`user_id 3.14\`", "v1.2.3", "REQ-123", "sha256:deadbeef", "12:34:56", "2026-11-10T12:34:56Z"]:
    assert literal in spoken, f"Protected literal changed: {literal} -> {spoken}"
assert "/a?x=1;" in spoken
print("OK")
`, 'Protected literal test'));

test('speaks_en_us_dates_with_ordinal_days', async () => assertPython(`
from voice.verbalizer import verbalize_for_speech as v
expected = "November ninth, twenty twenty-six"
assert v("November 09, 2026") == expected
assert v("11/09/2026") == expected
assert v("2026-11-09") == expected
assert v("February 29, 2024") == "February twenty-ninth, twenty twenty-four"
assert v("13/13/2026") == "13/13/2026"
print("OK")
`, 'Date normalization'));

test('speaks_clock_times_and_expands_weekdays_by_context', async () => assertPython(`
from voice.verbalizer import verbalize_for_speech as v
assert v("It is 11:32 PM.") == "It is eleven thirty-two p.m."
assert v("Set it for 6:05 AM.") == "Set it for six oh five a.m."
assert v("At 18:30") == "At eighteen thirty"
assert v("Open hours are 9 a.m.-5 p.m.") == "Open hours are nine a.m. to five p.m."
assert v("Blocked hours are 6 p.m.–8 p.m.") == "Blocked hours are six p.m. to eight p.m."
assert v("Every Mon, Tue, Wed") == "Every Monday, Tuesday, Wednesday"
assert v("Monument was quiet.") == "Monument was quiet."
assert v("Mon was quiet.") == "Mon was quiet."
assert v("Every Mon-Fri from 9:00 AM") == "Every Monday through Friday from nine a.m."
print("OK")
`, 'Time and weekday normalization'));

test('speaks_typed_numbers_and_preserves_protected_numeric_forms', async () => assertPython(`
from voice.verbalizer import verbalize_for_speech as v
assert v("42, 21st, 1.05, -3, 5-10") == "forty-two, twenty-first, one point zero five, minus three, five to ten"
assert v("1/2 and 3/4") == "one half and three quarters"
assert v("50/50, v1.2.3, 10/20/2026") == "50/50, v1.2.3, October twentieth, twenty twenty-six"
assert v("1th") == "1th"
assert v("007") == "007"
print("OK")
`, 'Numeric normalization'));

test('speaks_supported_measurement_families_without_converting_values', async () => assertPython(`
from voice.verbalizer import verbalize_for_speech as v
cases = {
    "1 µs": "one microsecond", "2 ms": "two milliseconds", "1 s": "one second", "2 sec": "two seconds",
    "1 min": "one minute", "1 h": "one hour", "2 hr": "two hours", "1 mm": "one millimeter",
    "2 cm": "two centimeters", "5 m long": "five meters long", "2 km": "two kilometers",
    "1 inch": "one inch", "2 in long": "two inches long", "1 ft": "one foot", "2 foot": "two feet",
    "1 yd": "one yard", "2 yard": "two yards", "1 mi": "one mile", "2 mile": "two miles",
    "1 mg": "one milligram", "2 g": "two grams", "1 kg": "one kilogram", "2 lb": "two pounds",
    "1 lbs": "one pound", "1 mL": "one milliliter", "2 L": "two liters", "1 cup": "one cup",
    "1 h 30 min": "one hour thirty minutes", "1.0 kg": "one point zero kilograms",
    "2 pt": "two pints", "1 qt": "one quart", "2 gal": "two gallons", "1 fl oz": "one fluid ounce",
    "80 km/h": "eighty kilometers per hour", "1 km/hr": "one kilometer per hour",
    "1 m/s": "one meter per second", "2 mph": "two miles per hour", "1°C": "one degree Celsius",
    "2°F": "two degrees Fahrenheit", "1 m²": "one square meter", "2 ft²": "two square feet",
    "1 sq ft": "one square foot", "2 sq mi": "two square miles"
}
for source, expected in cases.items():
    assert v(source) == expected, f"{source}: expected {expected}, got {v(source)}"
assert v("1.0 m") == "1.0 m"  # no value/unit guess for an ambiguous abbreviation
print("OK")
`, 'Measurement inventory'));

test('preserves_ambiguous_unit_abbreviations', async () => assertPython(`
from voice.verbalizer import verbalize_for_speech as v
assert v("5 m") == "5 m"
assert v("2 in the box") == "2 in the box"
assert v("8 oz") == "8 oz"
assert v("5 m long") == "five meters long"
assert v("for 5 m") == "for five minutes"
assert v("A specific duration (like 30m), open hours (for example, 9am-5pm)") == "A specific duration (like thirty minutes), open hours (for example, nine a.m. to five p.m.)"
assert v("The wall is 30m tall.") == "The wall is thirty meters tall."
assert v("The point is 30m away.") == "The point is thirty meters away."
assert v("The table is 30m by 20m.") == "The table is thirty meters by twenty meters."
assert v("30m") == "30m"
assert v("3 in long") == "three inches long"
assert v("8 oz of flour") == "eight ounces of flour"
assert v("5 xyz") == "5 xyz"
print("OK")
`, 'Ambiguous unit fallbacks'));

test('applies_abbreviations_and_slash_semantics_by_context', async () => assertPython(`
from voice.verbalizer import verbalize_for_speech as v
assert v("w/coffee and w/o sound") == "with coffee and without sound"
assert v("I/O device") == "input and output device"
assert v("cookies/milk") == "cookies or milk"
assert v("lock/logoff/shutdown") == "lock, logoff, or shutdown"
assert v("cookies and/or milk") == "cookies, milk, or both"
assert v("80 km/h") == "eighty kilometers per hour"
assert v("1/2, 50/50, 10/20/2026") == "one half, 50/50, October twentieth, twenty twenty-six"
assert v("I/O") == "I/O"
print("OK")
`, 'Abbreviation and slash rules'));

test('normalization_is_idempotent_and_fails_open', async () => assertPython(`
from unittest.mock import patch
import voice.verbalizer as module
raw = "1.5 kg, 11:32 PM, lock/logoff/shutdown."
spoken = module.verbalize_for_speech(raw)
assert module.verbalize_for_speech(spoken) == spoken
assert module.verbalize_for_speech("") == ""
with patch.object(module, "normalize_spoken_text", side_effect=RuntimeError("injected")):
    assert module.verbalize_for_speech(raw) == raw
print("OK")
`, 'Idempotence and fail-safe'));
