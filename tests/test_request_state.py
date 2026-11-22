"""Unit tests for ephemeral request fields and their provenance."""
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "voice"))
from request_state.state import RequestState
from session.conversation import Conversation


class RequestStateTests(unittest.TestCase):
    def test_stores_typed_fields_with_source_turn(self):
        state = RequestState()
        accepted, rejected = state.merge(
            "add_block", {"sites": ["reddit.com"], "schedule_type": "for_duration",
                          "duration": "45m"},
            {"sites": "reddit", "schedule_type": "for 45 minutes", "duration": "45 minutes"},
            "Block reddit for 45 minutes", "turn-1")
        self.assertEqual(accepted, ["sites", "schedule_type", "duration"])
        self.assertEqual(rejected, [])
        self.assertEqual(state.slots["duration"], "45m")
        self.assertEqual(state.provenance["duration"]["turn_id"], "turn-1")
        self.assertEqual(state.provenance["duration"]["kind"], "user_turn")
        self.assertEqual(state.provenance["target_kind"]["kind"], "derived")
        self.assertEqual(state.provenance["target_kind"]["turn_id"], "turn-1")

    def test_correction_updates_field_and_invalidates_dependent_values(self):
        state = RequestState()
        state.merge("add_block", {"sites": ["reddit.com"], "schedule_type": "blocked_hours",
                                   "blocked_hours": "22:00-23:00", "days": "mon,tue"}, {},
                    "Block reddit 10 to 11 weekdays", "turn-1", require_evidence=False)
        accepted, rejected = state.merge(
            "add_block", {"schedule_type": "for_duration", "duration": "45m"}, {},
            "Actually for 45 minutes", "turn-2", require_evidence=False)
        self.assertEqual(accepted, ["schedule_type", "duration"])
        self.assertEqual(rejected, [])
        self.assertEqual(state.slots["duration"], "45m")
        self.assertNotIn("blocked_hours", state.slots)
        self.assertNotIn("days", state.slots)
        self.assertEqual(state.provenance["schedule_type"]["turn_id"], "turn-2")

    def test_device_correction_clears_old_app_targets(self):
        state = RequestState()
        state.merge("add_block", {"apps": ["Chess"], "schedule_type": "always"}, {},
                    "Block Chess always", "turn-1", require_evidence=False)
        state.merge("add_block", {"device": "lock"}, {}, "Lock the whole computer", "turn-2",
                    require_evidence=False)
        self.assertEqual(state.slots["device"], "lock")
        self.assertNotIn("apps", state.slots)
        self.assertEqual(state.slots["target_kind"], "device")

    def test_status_target_type_change_clears_old_name(self):
        state = RequestState()
        state.merge("check_status", {"target_type": "app", "name": "Chess"},
                    {"target_type": "Chess app", "name": "Chess"}, "Check the Chess app",
                    "turn-1")
        state.merge("check_status", {"target_type": "site"},
                    {"target_type": "website"}, "Actually check the website", "turn-2")
        self.assertEqual(state.slots["target_type"], "site")
        self.assertNotIn("name", state.slots)

    def test_unquoted_patch_is_rejected_and_cancel_clears_state(self):
        state = RequestState()
        accepted, rejected = state.merge("start_session", {"length": "25m"},
                                         {"length": "2 hours"}, "Focus please", "turn-1")
        self.assertEqual(accepted, [])
        self.assertEqual(rejected, ["length"])
        self.assertEqual(state.action, "start_session")
        self.assertEqual(state.slots, {})
        state.merge("start_session", {"length": "25m"}, {"length": "25 minutes"},
                    "Focus for 25 minutes", "turn-2")
        state.clear()
        self.assertFalse(state.active)
        self.assertEqual(state.slots, {})
        self.assertEqual(state.provenance, {})

    def test_evidence_uses_word_boundaries_and_rejects_unparsed_duration(self):
        state = RequestState()
        accepted, rejected = state.merge("add_block", {"sites": ["red.com"]},
                                         {"sites": "red"}, "Block reddit", "turn-1")
        self.assertEqual(accepted, [])
        self.assertEqual(rejected, ["sites"])
        accepted, rejected = state.merge("start_session", {"length": "until 7pm"},
                                         {"length": "until 7pm"}, "Focus until 7pm", "turn-2")
        self.assertEqual(accepted, [])
        self.assertEqual(rejected, ["length"])

    def test_conversation_cancel_clears_active_request_without_persisting_it(self):
        conversation = Conversation(memory=object())
        conversation.active_request.merge("start_session", {"length": "25m"}, {},
                                          "Focus for 25 minutes", "turn-1", require_evidence=False)
        conversation.cancel()
        restarted = Conversation(memory=object())
        self.assertFalse(conversation.active_request.active)
        self.assertFalse(restarted.active_request.active)


if __name__ == "__main__":
    unittest.main()
