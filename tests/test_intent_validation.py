"""Safety checks for model-generated voice block intents."""
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "voice"))
from intent_validation import blocked_hours_to_open_hours, tool_to_cli_args, validate_intent


class IntentValidationTests(unittest.TestCase):
    def blocked(self, **overrides):
        args = {"name": "Night Lock", "device": "lock", "schedule_type": "blocked_hours",
                "blocked_hours": "00:00-07:00", "days": "mon,tue,wed,thu,fri,sat,sun"}
        args.update(overrides)
        return {"action": "tool_call", "tool": "add_block", "arguments": args}

    def test_blocked_window_is_inverted_for_cli_and_kept_in_user_intent(self):
        intent = validate_intent(self.blocked(), "Lock my computer every day from midnight to 7 a.m.")
        self.assertEqual(intent["arguments"]["blocked_hours"], "00:00-07:00")
        self.assertEqual(blocked_hours_to_open_hours("00:00-07:00"), "07:00-00:00")
        command = tool_to_cli_args("settings.zip", intent["tool"], intent["arguments"])
        self.assertEqual(command[command.index("--open-hours") + 1], "07:00-00:00")
        self.assertEqual(command[command.index("--days") + 1], "mon,tue,wed,thu,fri,sat,sun")
        self.assertNotIn("--write", command)

    def test_missing_days_requires_clarification_unless_user_said_daily(self):
        intent = validate_intent(self.blocked(days=None), "Lock my computer from midnight to 7 a.m.")
        self.assertEqual(intent["action"], "reply")
        self.assertIn("Which days", intent["content"])
        daily = validate_intent(self.blocked(days=None), "Lock my computer every day from midnight to 7 a.m.")
        self.assertEqual(daily["arguments"]["days"], "mon,tue,wed,thu,fri,sat,sun")

    def test_explicit_weekdays_map_without_carrying_hours_into_other_days(self):
        intent = validate_intent(self.blocked(blocked_hours="04:00-07:00", days="mon,tue,wed,thu,fri"),
                                 "Lock my computer every weekday from 4 a.m. to 7 a.m.")
        self.assertEqual(intent["action"], "tool_call")
        command = tool_to_cli_args("settings.zip", intent["tool"], intent["arguments"])
        self.assertEqual(command[command.index("--open-hours") + 1], "00:00-04:00,07:00-24:00")
        self.assertEqual(command[command.index("--days") + 1], "mon,tue,wed,thu,fri")

    def test_model_days_cannot_override_user_recurrence(self):
        intent = validate_intent(self.blocked(days="mon,tue,wed,thu,fri"),
                                 "Lock my computer every day from midnight to 7 a.m.")
        self.assertEqual(intent["action"], "reply")
        self.assertIn("do not match", intent["content"])

    def test_selected_days_with_overnight_window_requires_daily_schedule(self):
        intent = self.blocked(blocked_hours="22:00-06:00", days="mon,tue,wed,thu,fri")
        result = validate_intent(intent, "Block my computer every weekday from 10 p.m. to 6 a.m.")
        self.assertEqual(result["action"], "reply")
        self.assertIn("crosses midnight", result["content"])

    def test_wrong_open_hours_direction_is_stopped(self):
        intent = {"action": "tool_call", "tool": "add_block", "arguments": {
            "name": "Night Lock", "device": "lock", "schedule_type": "open_hours",
            "open_hours": "00:00-07:00"}}
        for text in ("Lock the computer so I cannot use it from midnight to 7 a.m.",
                     "I don't want to be able to use my computer from midnight to 7 a.m.",
                     "I want the computer not to be usable from midnight to 7 a.m."):
            result = validate_intent(intent, text)
            self.assertEqual(result["action"], "reply")
            self.assertIn("sounds like a blocked time", result["content"])

    def test_explicit_allowed_window_stays_open_hours(self):
        intent = {"action": "tool_call", "tool": "add_block", "arguments": {
            "name": "Work Hours", "sites": ["reddit.com"], "schedule_type": "open_hours",
            "open_hours": "09:00-17:00"}}
        self.assertEqual(validate_intent(intent, "Keep reddit usable from 9 a.m. to 5 p.m.")["action"], "tool_call")

    def test_invalid_window_or_mixed_target_does_not_stage(self):
        for intent in (self.blocked(blocked_hours="25:00-07:00"),
                       self.blocked(apps=["com.example.app"])):
            self.assertEqual(validate_intent(intent, "Lock the computer every day")["action"], "reply")

    def test_clock_range_cannot_be_misread_as_a_relative_duration(self):
        intent = {"action": "tool_call", "tool": "add_block", "arguments": {
            "name": "Night Lock", "device": "lock", "schedule_type": "for_duration", "duration": "7h"}}
        result = validate_intent(intent, "Lock my computer every day from midnight until 7 a.m.")
        self.assertEqual(result["action"], "reply")
        self.assertIn("repeating blocked-hours", result["content"])

    def test_cli_mapper_never_turns_unknown_schedule_into_always(self):
        with self.assertRaises(ValueError):
            tool_to_cli_args("settings.zip", "add_block", {
                "name": "bad", "apps": ["com.example"], "schedule_type": "unknown"})


if __name__ == "__main__":
    unittest.main()
