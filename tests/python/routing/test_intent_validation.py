"""Safety checks for canonical voice intents and native CLI arguments."""
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[3]))
from voice.routing.intent_validation import tool_to_cli_args, validate_intent
from voice.routing.native_cli import PLANNED_TOOLS, tool_to_command


class IntentValidationTests(unittest.TestCase):
    def test_legacy_internal_names_map_to_canonical_tools(self):
        for legacy, canonical in (("add_block", "create_rule"),
                                  ("start_session", "request_pass"),
                                  ("switch_off", "delete_rule")):
            intent = validate_intent({"action": "tool_call", "tool": legacy,
                                      "arguments": {"name": "Study", "id": "r1"}}, "request")
            self.assertEqual(intent["tool"], canonical)

    def test_missing_id_requires_clarification(self):
        idless = validate_intent({"action": "tool_call", "tool": "show_rule",
                                  "arguments": {}}, "show")
        self.assertEqual(idless["action"], "reply")
        self.assertIn("id", idless["content"])

    def test_argv_uses_only_canonical_commands_and_flags(self):
        command = tool_to_cli_args("store.json", "create_rule",
                                   {"name": "Study", "sites": ["reddit.com"]})
        self.assertEqual(command[0], "create-rule")
        self.assertEqual(command[command.index("--store") + 1], "store.json")
        self.assertIn("--json", command)
        for flag in ("--open-hours", "--wall", "--write", "--blocked-hours", "--for", "--days"):
            self.assertNotIn(flag, command)
        target = command[command.index("--target") + 1]
        self.assertIn("specific_websites", target)

    def test_injected_timestamp_is_passed_through(self):
        command = tool_to_cli_args("store.json", "check_status",
                                   {"id": "r1", "at": "2026-01-01T12:00:00Z"})
        self.assertEqual(command[command.index("--at") + 1], "2026-01-01T12:00:00Z")

    def test_planned_tools_have_no_cli_command(self):
        for tool in PLANNED_TOOLS:
            self.assertIsNone(tool_to_command(tool))
            self.assertEqual(tool_to_cli_args("store.json", tool, {}), [])


if __name__ == "__main__":
    unittest.main()
