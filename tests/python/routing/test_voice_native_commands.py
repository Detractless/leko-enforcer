"""Native CLI command mapping and deterministic routing checks."""

import os
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT))

from voice.routing import native_cli
from voice.routing.intent_validation import tool_to_cli_args, validate_intent
from voice.routing.llm_router import TOOLS


LEGACY_COMMANDS = {"add-block", "session", "switch-off", "status"}
LEGACY_FLAGS = {"--open-hours", "--wall", "--write", "--blocked-hours", "--for", "--days"}


class VoiceNativeCommandTests(unittest.TestCase):
    def test_voice_tools_map_only_to_canonical_cli_commands(self):
        names = {entry["function"]["name"] for entry in TOOLS}
        self.assertEqual(names, set(native_cli.CANONICAL_TOOLS))
        for tool in native_cli.CANONICAL_TOOLS:
            argv = native_cli.build_argv("store.json", tool, {"id": "r1", "name": "Study"})
            self.assertIn(argv[0], native_cli.CANONICAL_TOOLS.values())
            self.assertNotIn(argv[0], LEGACY_COMMANDS)
            for part in argv:
                if part.startswith("--"):
                    self.assertNotIn(part, LEGACY_FLAGS)
                    self.assertNotIn("_", part)
        self.assertIn("--json", native_cli.build_argv("store.json", "check_status", {"id": "r1"}))

    def test_no_legacy_hours_or_legacy_vocabulary_remain(self):
        for path in (ROOT / "voice/routing").glob("*.py"):
            text = path.read_text(encoding="utf-8")
            for legacy in ("blocked_hours_to_open_hours", "--open-hours", "--wall",
                           '"add-block"', '"switch-off"', '"session"'):
                self.assertNotIn(legacy, text, "{} still has {}".format(path.name, legacy))
        intent = validate_intent({"action": "tool_call", "tool": "add_block",
                                  "arguments": {"name": "Study"}}, "block study")
        self.assertEqual(intent["tool"], "create_rule")

    def test_tests_use_a_temp_store_and_an_injected_timestamp(self):
        with tempfile.TemporaryDirectory() as folder:
            store = Path(folder) / "rules.json"
            argv = native_cli.build_argv(str(store), "check_status",
                                         {"id": "r1"}, at="2026-01-01T12:00:00Z")
            self.assertEqual(argv[argv.index("--store") + 1], str(store))
            self.assertEqual(argv[argv.index("--at") + 1], "2026-01-01T12:00:00Z")
            same = tool_to_cli_args(str(store), "check_status",
                                    {"id": "r1", "at": "2026-01-01T12:00:00Z"})
            self.assertEqual(argv, same)

    def test_missing_or_wrong_inputs_fail_cleanly(self):
        with self.assertRaises(ValueError):
            native_cli.tool_to_command("add_block_not_a_tool")
        self.assertIsNone(native_cli.tool_to_command("pomodoro_schedule"))
        reply = validate_intent({"action": "tool_call", "tool": "show_rule",
                                 "arguments": {}}, "show it")
        self.assertEqual(reply["action"], "reply")
        self.assertIn("id", reply["content"])

    def test_repeated_spoken_commands_are_idempotent(self):
        first = validate_intent({"action": "tool_call", "tool": "create_rule",
                                 "arguments": {"name": "Study"}}, "create a rule")
        second = validate_intent({"action": "tool_call", "tool": "create_rule",
                                  "arguments": {"name": "Study"}}, "create a rule")
        self.assertEqual(first, second)
        argv = tool_to_cli_args("store.json", "create_rule", {"name": "Study"})
        self.assertEqual(argv, tool_to_cli_args("store.json", "create_rule", {"name": "Study"}))


if __name__ == "__main__":
    unittest.main()
