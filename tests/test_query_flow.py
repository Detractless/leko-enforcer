"""Offline conversation-flow checks using fake model and CLI adapters."""
import unittest
from unittest.mock import patch
import test_conversation as fixtures


class QueryFlowTests(unittest.TestCase):
    setUp = fixtures.ConversationTests.setUp
    query = fixtures.ConversationTests.query
    run_tool = fixtures.ConversationTests.run_tool
    writes = fixtures.ConversationTests.writes

    def test_asks_one_missing_field_and_skips_volunteered_fields(self):
        updates = [
            {"action": "request_update", "intent": "add_block", "fields": {"sites": ["reddit.com"]},
             "evidence": {"sites": "reddit"}},
            {"action": "request_update", "intent": "add_block",
             "fields": {"schedule_type": "for_duration", "duration": "30m"},
             "evidence": {"schedule_type": "for 30 minutes", "duration": "30 minutes"}},
        ]
        with patch.object(fixtures.fake_voice, "parse_request_update_safe", create=True,
                          side_effect=updates) as model:
            first = self.query("Block reddit", turn_id="turn-1")
            self.assertIn("Should the block apply", first)
            self.assertEqual(self.state.active_request.last_question_field, "schedule_type")
            self.assertIsNone(self.state.pending)
            second = self.query("For 30 minutes", turn_id="turn-2")

        self.assertIn("reddit.com", second)
        self.assertIn("30m", second)
        self.assertEqual(model.call_count, 2)
        self.assertEqual(self.state.pending["intent"]["arguments"]["duration"], "30m")
        self.assertEqual(self.state.active_request.provenance["sites"]["turn_id"], "turn-1")
        self.assertFalse(self.writes())

    def test_complete_first_turn_skips_all_clarifications(self):
        update = {"action": "request_update", "intent": "start_session",
                  "fields": {"length": "25m"}, "evidence": {"length": "25 minutes"}}
        with patch.object(fixtures.fake_voice, "parse_request_update_safe", create=True,
                          return_value=update):
            response = self.query("Focus for 25 minutes")
        self.assertIn("Prepared a focus session of 25m", response)
        self.assertEqual(self.state.pending["intent"]["arguments"]["length"], "25m")
        self.assertEqual(self.state.active_request.last_question_field, None)

    def test_invalid_time_is_clarified_before_missing_recurrence(self):
        update = {"action": "request_update", "intent": "add_block",
                  "fields": {"sites": ["reddit.com"], "schedule_type": "blocked_hours",
                             "blocked_hours": "25:00-07:00"},
                  "evidence": {"sites": "reddit", "schedule_type": "from 25:00 to 7",
                               "blocked_hours": "from 25:00 to 7"}}
        with patch.object(fixtures.fake_voice, "parse_request_update_safe", create=True,
                          return_value=update):
            response = self.query("Block reddit from 25:00 to 7")
        self.assertIn("exact start and end times", response)
        self.assertEqual(self.state.active_request.last_question_field, "blocked_hours")
        self.assertIsNone(self.state.pending)

    def test_unsupported_state_echo_cannot_restale_a_completed_request(self):
        complete = {"action": "request_update", "intent": "add_block",
                    "fields": {"sites": ["reddit.com"], "schedule_type": "for_duration",
                               "duration": "30m"},
                    "evidence": {"sites": "reddit", "schedule_type": "30 minutes",
                                 "duration": "30 minutes"}}
        echo = {"action": "request_update", "intent": "add_block",
                "fields": {"sites": ["reddit.com"], "schedule_type": "for_duration",
                           "duration": "30m"}, "evidence": {}}
        with patch.object(fixtures.fake_voice, "parse_request_update_safe", side_effect=[complete, echo],
                          create=True):
            self.query("Block reddit for 30 minutes")
            response = self.query("What time is it?")
        self.assertIn("didn't catch a new detail", response)
        self.assertIsNone(self.state.pending)
        self.assertEqual(self.state.active_request.slots["duration"], "30m")

    def test_invalid_fields_for_each_action_never_reach_cli_preview(self):
        updates = [
            {"action": "request_update", "intent": "start_session",
             "fields": {"length": "0m"}, "evidence": {"length": "zero minutes"}},
            {"action": "request_update", "intent": "switch_off",
             "fields": {"id": " "}, "evidence": {"id": "that block"}},
            {"action": "request_update", "intent": "check_status",
             "fields": {"target_type": "app"}, "evidence": {"target_type": "an app"}},
            {"action": "request_update", "intent": "add_block",
             "fields": {"device": "lock", "sites": ["reddit.com"]},
             "evidence": {"device": "whole computer", "sites": "reddit"}},
        ]
        utterances = ["Focus for zero minutes", "Turn off that block", "Check an app",
                      "Lock the whole computer and block reddit"]
        with patch.object(fixtures.fake_voice, "parse_request_update_safe", side_effect=updates,
                          create=True):
            replies = [self.query(text) for text in utterances]
        self.assertIn("How long", replies[0])
        self.assertIn("Which block", replies[1])
        self.assertIn("Which app or website", replies[2])
        self.assertIn("whole computer", replies[3])
        self.assertIsNone(self.state.pending)
        self.assertEqual(self.calls, [])


if __name__ == "__main__":
    unittest.main()
