"""Offline conversation-flow checks using fake model and CLI adapters."""
import unittest
from unittest.mock import patch
import python.session.test_conversation as fixtures


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

        self.assertIn("Technology Management Rule", second)
        self.assertEqual(model.call_count, 2)
        self.assertEqual(self.state.pending["intent"]["arguments"]["duration"], "30m")
        self.assertEqual(self.state.active_request.provenance["sites"]["turn_id"], "turn-1")
        self.assertFalse(self.writes())

    def test_complete_first_turn_skips_all_clarifications(self):
        intent = {"action": "tool_call", "tool": "request_pass",
                  "arguments": {"id": "r1", "length": "25m"}}
        with patch.object(fixtures.fake_voice, "parse_user_intent_safe", create=True,
                          return_value=intent):
            response = self.query("Request a pass for rule r1")
        self.assertIn("Prepared a Pass request", response)
        self.assertEqual(self.state.pending["intent"]["tool"], "request_pass")
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

    def test_deterministic_voice_routes_queries_and_emits_reply_card(self):
        import os
        from voice.routing import deterministic_router, llm_request
        with patch.dict(os.environ, {}, clear=False):
            os.environ.pop("LEKO_ENABLE_LLM_ROUTER", None)
            with patch.object(fixtures.fake_voice, "parse_request_update_safe",
                              side_effect=lambda q, **_kw: llm_request.parse_request_update_safe(q), create=True):
                reply = self.query("tell me a joke about bananas")
                self.assertEqual(reply, deterministic_router.FALLBACK_REPLY)
                card_events = [e for e in self.events if e.get("type") == "reply_card"]
                self.assertTrue(card_events)
                self.assertEqual(card_events[-1]["text"], deterministic_router.FALLBACK_REPLY)

                def fake_cli(cmd, **_kw):
                    self.calls.append(cmd)
                    if "countdowns" in cmd:
                        return type("P", (), {"returncode": 0, "stdout": '{"countdowns":[{"rule_id":"r1","rule":"Study","kind":"Time Limit","transition":"Limit Reached","remaining_seconds":300,"urgency":1}]}', "stderr": ""})()
                    return type("P", (), {"returncode": 0, "stdout": "ok", "stderr": ""})()

                with patch("voice.session.query_flow.subprocess.run", side_effect=fake_cli):
                    c_reply = self.query("what countdowns are active")
                self.assertIn("Study Time Limit", c_reply)
                card = [e for e in self.events if e.get("type") == "reply_card"][-1]
                self.assertEqual(card["columns"], ["Rule", "Kind", "Remaining"])
                self.assertEqual(card["rows"], [["Study", "Time Limit", "05:00"]])

    def test_reply_card_and_speaking_wait_for_playback_start_and_send_duration_ms(self):
        from voice.core.ui_bridge import compute_word_timings_ms
        import numpy as np

        sr = 24000
        pcm = np.zeros(int(sr * 2.0), dtype=np.float32)
        pcm[int(sr * 0.2):int(sr * 0.8)] = 0.2
        pcm[int(sr * 1.1):int(sr * 1.6)] = 0.2
        timings = compute_word_timings_ms(pcm, sr, "Hello there, friend!", lead_ms=40)
        self.assertEqual(len(timings), 3)
        self.assertEqual(timings[0], 240)
        self.assertGreater(timings[1], timings[0])
        self.assertEqual(timings[2], 1140)

        observed_before_start = []

        def fake_speak(text, cancel=None, on_playback_start=None):
            observed_before_start.extend(e.get("type") or e.get("state") for e in self.events)
            if on_playback_start:
                on_playback_start(2.45, timings)

        self.intent = {"action": "reply", "content": "Hello there, friend!"}
        self.query("hi", speak_fn=fake_speak)
        self.assertNotIn("reply_card", observed_before_start)
        speaking = [e for e in self.events if e.get("state") == "speaking"]
        self.assertEqual(len(speaking), 1)
        self.assertEqual(speaking[0]["duration_ms"], 2450)
        self.assertEqual(speaking[0]["word_timings_ms"], timings)
        card_idx = next(i for i, e in enumerate(self.events) if e.get("type") == "reply_card")
        spk_idx = next(i for i, e in enumerate(self.events) if e.get("state") == "speaking")
        self.assertLess(card_idx, spk_idx)


if __name__ == "__main__":
    unittest.main()
