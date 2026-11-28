"""Tests for reverse spoken-to-written normalization (ITN) and router interoperability."""
import unittest

from voice.normalization.speech_text import normalize_spoken_text
from voice.stt.spoken_to_written import normalize_written_text
from voice.routing.deterministic_router import classify_text


class SpokenToWrittenTests(unittest.TestCase):
    def test_dates_times_and_years_from_kroko_samples(self):
        self.assertEqual(
            normalize_written_text(
                "Thank you. The simulated appointment is confirmed for Tuesday, "
                "November third, two thousand and twenty-six, at nine thirty AM. Goodbye."
            ),
            "Thank you. The simulated appointment is confirmed for Tuesday, "
            "November 3, 2026, at 9:30 a.m. Goodbye.",
        )
        self.assertEqual(
            normalize_written_text("Thank you. Tuesday, November third, twenty twenty-six at nine oh five pm."),
            "Thank you. Tuesday, November 3, 2026 at 9:05 p.m.",
        )
        self.assertEqual(
            normalize_written_text("Set hours from nine AM to five PM and nine o'clock."),
            "Set hours from 9 a.m. to 5 p.m. and 9:00.",
        )
        self.assertEqual(normalize_written_text("Meet at nine thirty"), "Meet at 9:30")

    def test_round_trip_with_speech_text(self):
        for written in ("9:30 a.m.", "November 3, 2026", "30 minutes", "3.5", "2 hours"):
            spoken = normalize_spoken_text(written)
            self.assertEqual(normalize_written_text(spoken).lower(), written.lower())

    def test_numbers_decimals_and_capitalization(self):
        self.assertEqual(
            normalize_written_text(
                "my apologies. always, a specific duration, like twenty-five minutes, "
                "or nine AM five PM, or three point five hours."
            ),
            "My apologies. Always, a specific duration, like 25 minutes, "
            "or 9 a.m. 5 p.m., or 3.5 hours.",
        )
        self.assertEqual(normalize_written_text("Wait for one hour and two hundred blocks"), "Wait for 1 hour and 200 blocks")

    def test_router_understands_normalized_spoken_duration(self):
        normalized = normalize_written_text("block reddit.com for thirty minutes")
        self.assertEqual(normalized, "Block reddit.com for 30 minutes")
        routed = classify_text(normalized)
        self.assertEqual(routed["tool"], "create_rule")
        self.assertEqual(routed["arguments"]["duration"], "30m")

    def test_guards_conversational_small_words_domains_and_phonon_idempotency(self):
        self.assertEqual(
            normalize_written_text("Which one should we may one day block on reddit.com?"),
            "Which one should we may one day block on reddit.com?",
        )
        self.assertEqual(normalize_written_text("reddit.com works at 9.30 a.m."), "reddit.com works at 9.30 a.m.")
        phonon = "Thank you. The simulated appointment is confirmed for Tuesday, November 3, 2026, at 9.30 a.m. Goodbye."
        self.assertEqual(normalize_written_text(phonon), phonon)


if __name__ == "__main__":
    unittest.main()
