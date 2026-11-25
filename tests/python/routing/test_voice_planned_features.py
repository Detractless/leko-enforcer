"""Planned-feature replies must never reach the native CLI."""

import sys
import unittest
from pathlib import Path
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT))

from voice.routing.canonical_speech import PLANNED_NAMES, PLANNED_REPLY, planned_reply
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


if __name__ == "__main__":
    unittest.main()
