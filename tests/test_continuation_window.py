"""Sample-clock endpoint tests using real capture gating; no devices or models."""
import unittest
from unittest.mock import Mock

import numpy as np
import test_capture_gate as capture_fixtures
from test_capture_gate import eventually


class ContinuationTests(unittest.TestCase):
    setUp = capture_fixtures.CaptureTests.setUp

    def acoustic_endpoint(self):
        detector = self.gate.detector
        detector.sess = Mock()
        detector.sess.run.return_value = [np.array([0.99])]
        detector.state, detector.shapes, detector.out = {}, {}, ["probs"]
        return detector

    def feed(self, value, frames=1):
        for _ in range(frames):
            self.streams[-1].feed(np.full(2560, value))

    def test_resumed_speech_is_one_submission_with_both_segments(self):
        detector = self.acoustic_endpoint()
        self.feed(5000)
        self.feed(0, 5)  # 800ms: acoustic endpoint seen, window still open.
        self.assertTrue(detector.endpoint_pending)
        self.assertFalse(self.turns)
        self.assertFalse(self.gate.busy)
        self.assertTrue(self.streams[0].active)
        self.feed(9000)
        self.assertFalse(detector.endpoint_pending)
        self.feed(0, 5)
        self.assertFalse(self.turns)
        self.feed(0)
        self.assertEqual(len(self.turns), 1)
        expected = np.concatenate([np.full(2560, 5000), np.zeros(2560 * 5),
                                   np.full(2560, 9000), np.zeros(2560 * 6)]) / 32768
        np.testing.assert_array_equal(self.turns[0][0], expected)
        self.assertEqual(len(self.onsets), 1)

    def test_quiet_endpoint_waits_then_submits_once_and_gates_response(self):
        detector = self.acoustic_endpoint()
        self.feed(5000)
        self.feed(0, 4)
        # Acoustic evidence need not remain high while the window expires.
        detector.sess.run.return_value = [np.array([0.0])]
        self.feed(0)
        self.assertFalse(self.turns)
        self.assertFalse(self.gate.busy)
        self.feed(0)  # Sixth quiet frame = 960ms, first boundary >= 900ms.
        self.assertEqual(len(self.turns), 1)
        self.assertTrue(self.gate.busy)
        eventually(lambda: self.streams[0].closed)
        self.feed(12000, 20)
        self.assertEqual(len(self.turns), 1)
        self.assertFalse(detector.buf)
        self.assertFalse(detector.endpoint_pending)
        self.assertEqual(detector.quiet_samples, 0)

    def test_fallback_does_not_add_window_to_existing_1280ms(self):
        self.feed(5000)
        self.feed(0, 7)
        self.assertFalse(self.turns)
        self.feed(0)
        self.assertEqual(len(self.turns), 1)

    def test_pending_window_cleared_by_mute_cancel_reset_and_shutdown(self):
        self.acoustic_endpoint()
        for action in ('mute', 'cancel', 'shutdown'):
            with self.subTest(action=action):
                self.feed(5000)
                self.feed(0, 5)
                old = self.streams[-1]
                count = len(self.streams)
                if action == 'shutdown':
                    self.gate.stop()
                else:
                    # Runtime stop/cancel uses set_muted(current_mute_state).
                    self.gate.set_muted(action == 'mute')
                eventually(lambda: old.closed)
                old.feed(np.zeros(2560))
                self.assertFalse(self.turns)
                self.assertFalse(self.gate.detector.endpoint_pending)
                self.assertFalse(self.gate.detector.buf)
                self.assertFalse(self.gate._raw_buf)
                if action != 'shutdown':
                    if action == 'mute':
                        self.gate.set_muted(False)
                    eventually(lambda: len(self.streams) > count and self.streams[-1].active)

    def test_typed_response_discards_pending_audio_immediately(self):
        self.acoustic_endpoint()
        self.feed(5000)
        self.feed(0, 5)
        token = self.gate.begin_response()
        self.assertIsNotNone(token)
        eventually(lambda: self.streams[0].closed)
        self.assertFalse(self.gate.detector.endpoint_pending)
        self.assertFalse(self.gate.detector.buf)
        self.assertFalse(self.turns)


if __name__ == '__main__':
    unittest.main()
