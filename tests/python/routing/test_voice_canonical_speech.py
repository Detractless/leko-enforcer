"""Exact Title Case canonical speech checks."""

import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT))

from voice.routing import canonical_speech
from voice.request_state.draft_summary import draft_summary


class VoiceCanonicalSpeechTests(unittest.TestCase):
    def test_spoken_replies_use_exact_title_case_vocabulary(self):
        self.assertEqual(canonical_speech.canonical_name("technology_management_rule"),
                         "Technology Management Rule")
        self.assertEqual(canonical_speech.canonical_name("force_close_penalty"),
                         "Force Close Penalty")
        self.assertEqual(canonical_speech.canonical_name("queued_delay_seal"),
                         "Queued Delay Seal")
        self.assertEqual(canonical_speech.canonical_name("whole_device"), "Whole Device")
        self.assertEqual(canonical_speech.canonical_name("routine_schedule"), "Routine Schedule")
        self.assertEqual(canonical_speech.canonical_name("timed_pass"), "Timed Pass")
        self.assertEqual(canonical_speech.canonical_name("no_seal"), "No Seal")
        spoken = canonical_speech.status_sentence({"rule": "Study", "enforced": False,
                                                   "reason": "OutsideSchedule"})
        self.assertIn("Not Enforced", spoken)
        self.assertIn("Outside Schedule", spoken)

    def test_a_seal_refusal_is_spoken_as_blocked_not_edited(self):
        text = canonical_speech.seal_refusal("countdown_seal")
        self.assertEqual(text, "That change was blocked by the Countdown Seal.")
        self.assertNotIn("saved", text.lower())
        self.assertNotIn("edited", text.lower())

    def test_queued_and_cancelled_edits_are_spoken_accurately(self):
        queued = canonical_speech.queued_notice(45)
        self.assertIn("Queued Delay Seal", queued)
        self.assertIn("45", queued)
        self.assertIn("cancelled", canonical_speech.cancelled_notice())

    def test_an_earned_pass_is_not_reported_as_a_rule_edit(self):
        text = canonical_speech.pass_notice()
        self.assertIn("pass was earned", text)
        self.assertIn("not edited", text)
        summary, _, _, _ = draft_summary("request_pass", {}, "{}", lambda text, *_: text)
        self.assertIn("not be edited", summary)

    def test_empty_loading_and_error_states_are_spoken(self):
        self.assertIn("No rules", canonical_speech.empty_notice())
        self.assertIn("still loading", canonical_speech.loading_notice())
        self.assertIn("could not", canonical_speech.error_notice("bad store"))
        self.assertEqual(canonical_speech.format_status_summary("not json"),
                         "That status could not be read.")

    def test_all_actions_are_completable_by_voice_alone(self):
        for tool in ("create_rule", "list_rules", "show_rule", "edit_rule", "delete_rule",
                     "queue_edit", "cancel_edit", "request_pass", "check_status", "evaluate_rule"):
            summary, name, kind, description = draft_summary(tool, {"id": "r1"}, "{}",
                                                              lambda text, *_: text)
            self.assertTrue(summary)
            self.assertTrue(name)
            self.assertTrue(kind)
            self.assertTrue(description)

    def test_spoken_status_uses_persisted_pass_state(self):
        status = canonical_speech.status_sentence({
            "rule": "Study",
            "enforced": False,
            "reason": "PassActive",
            "pass": {"active": True},
        })
        self.assertIn("Not Enforced", status)
        self.assertIn("Pass Active", status)
        self.assertIn("not edited", status)
        plain = canonical_speech.status_sentence({
            "rule": "Study", "enforced": True, "reason": "Enforced", "pass": {"active": False},
        })
        self.assertNotIn("Earn Pass is active", plain)


if __name__ == "__main__":
    unittest.main()
