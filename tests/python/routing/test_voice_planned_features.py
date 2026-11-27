"""Planned-feature replies must never reach the native CLI."""

import sys
import unittest
from pathlib import Path
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT))

from voice.routing.canonical_speech import PLANNED_NAMES, PLANNED_REPLY, canonical_name, planned_reply
from voice.routing.native_cli import PLANNED_TOOLS, build_argv
import voice.session.query_flow as query_flow
from voice.session.conversation import Conversation


class VoicePlannedFeatureTests(unittest.TestCase):
    def test_planned_features_reply_without_any_cli_call(self):
        for name in PLANNED_NAMES:
            self.assertEqual(planned_reply("Can I use {}?".format(name)), PLANNED_REPLY)
        self.assertEqual(planned_reply("Create a rule"), None)

    def test_planned_request_does_not_call_the_llm_or_cli(self):
        with patch.object(query_flow, "_llm_intent",
                          side_effect=AssertionError("planned request reached the model")):
            with patch.object(query_flow.subprocess, "run",
                              side_effect=AssertionError("planned request reached the CLI")):
                answer = query_flow.process_query(
                    "Can I set up Pomodoro Schedule?",
                    "store.json",
                    conversation=Conversation(),
                    speak_fn=lambda text, cancel=None: None,
                )
        self.assertEqual(answer, PLANNED_REPLY)

    def engagement_pass_is_live_while_other_planned_features_refuse(self):
        self.assertNotIn("Engagement Pass", PLANNED_NAMES)
        self.assertNotIn("engagement_pass", PLANNED_TOOLS)
        self.assertIsNone(planned_reply("Earn an Engagement Pass on Notes"))
        self.assertEqual(canonical_name("engagement_pass"), "Engagement Pass")
        argv = build_argv(
            "store.json",
            "create_rule",
            {
                "name": "Earn",
                "apps": ["com.example.Bad"],
                "engagement_pass": True,
                "engagement_targets": ["com.apple.Notes"],
                "engagement_seconds": 120,
            },
        )
        self.assertIn("--engagement-pass", argv)
        self.assertIn("--engagement-targets", argv)
        self.assertIn("com.apple.Notes", argv)

        remaining = (
            "Pomodoro Schedule",
            "Community-Linked Apps",
            "Community-Linked Websites",
            "Restart Seal",
        )
        self.assertEqual(set(PLANNED_NAMES), set(remaining))
        for name in remaining:
            with patch.object(query_flow, "_llm_intent",
                              side_effect=AssertionError("planned request reached the model")):
                with patch.object(query_flow.subprocess, "run",
                                  side_effect=AssertionError("planned request reached the CLI")):
                    answer = query_flow.process_query(
                        f"Can I use {name}?",
                        "store.json",
                        conversation=Conversation(),
                        speak_fn=lambda text, cancel=None: None,
                    )
            self.assertEqual(answer, PLANNED_REPLY)

    def test_engagement_pass_is_live_while_other_planned_features_refuse(self):
        self.engagement_pass_is_live_while_other_planned_features_refuse()

    def test_planned_features_refuse_without_cli_or_llm(self):
        from voice.core.ui_bridge import UIPipelineBridge
        events = []
        bridge = UIPipelineBridge(event_sink=events.append)
        for name in PLANNED_NAMES:
            with patch.object(query_flow, "_llm_intent",
                              side_effect=AssertionError("planned request reached the model")), \
                 patch.object(query_flow.subprocess, "run",
                              side_effect=AssertionError("planned request reached the CLI")):
                answer = query_flow.process_query(
                    f"Tell me about {name}",
                    "store.json",
                    bridge=bridge,
                    conversation=Conversation(),
                    speak_fn=lambda text, cancel=None: None,
                )
            self.assertEqual(answer, PLANNED_REPLY)
        self.assertEqual(sum(1 for e in events if e.get("type") == "reply_card"), len(PLANNED_NAMES))


if __name__ == "__main__":
    unittest.main()
