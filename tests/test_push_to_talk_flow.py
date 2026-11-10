"""Exercise deliberate capture, grace merging, busy interruption, and safe failures."""
import sys
import tempfile
import threading
import time
import unittest
from unittest.mock import patch
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import Mock

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "voice"))
from audio_io.push_to_talk.push_to_talk import PushToTalkDetector, PushToTalkTurnManager
from session.session_log import SessionLog


def eventually(predicate, timeout=2):
    deadline = time.monotonic() + timeout
    while not predicate():
        if time.monotonic() >= deadline:
            raise AssertionError("Timed out waiting for push-to-talk lifecycle")
        time.sleep(0.002)


class FakeStream:
    def __init__(self, callback, **_kwargs):
        self.callback = callback
        self.active = False
        self.closed = False

    def start(self):
        self.active = True

    def stop(self):
        self.active = False

    def close(self):
        self.closed = True

    def feed(self, samples):
        values = np.asarray(samples, dtype=np.int16)
        self.callback(values.tobytes(), len(values), None, None)


class FakeEcho:
    available = True

    def __init__(self):
        self.cleaned = 0

    def clean_capture(self, samples):
        self.cleaned += 1
        return samples

    def reset_buffers(self):
        pass


class PushToTalkFlowTests(unittest.TestCase):
    def setUp(self):
        self.streams, self.turns = [], []
        self.echo = FakeEcho()

        def factory(**kwargs):
            stream = FakeStream(**kwargs)
            self.streams.append(stream)
            return stream

        self.manager = PushToTalkTurnManager(
            factory, self.echo, lambda audio, token: self.turns.append((audio.copy(), token)),
            sample_rate=16000, chunk_size=4, grace_s=0.12)
        self.manager.start()
        eventually(lambda: self.streams and self.streams[-1].active)
        self.addCleanup(self.manager.stop)

    def test_idle_audio_is_discarded_until_hold_or_release_grace(self):
        self.streams[-1].feed(np.full(4, 12000, dtype=np.int16))
        self.assertEqual(self.echo.cleaned, 0)
        self.assertEqual(self.turns, [])
        self.assertTrue(self.manager.begin_capture())
        self.streams[-1].feed(np.full(4, 12000, dtype=np.int16))
        self.assertEqual(self.echo.cleaned, 1)

    def test_release_grace_merges_rehold_and_submits_once_after_two_seconds(self):
        self.assertEqual(PushToTalkDetector().grace_s, 2.0)
        self.assertTrue(self.manager.begin_capture())
        self.streams[-1].feed(np.full(4, 4000, dtype=np.int16))
        self.assertTrue(self.manager.end_capture())
        time.sleep(0.03)
        self.assertTrue(self.manager.begin_capture())
        self.streams[-1].feed(np.full(4, 8000, dtype=np.int16))
        self.assertTrue(self.manager.end_capture())
        eventually(lambda: len(self.turns) == 1)
        self.assertEqual(len(self.turns[0][0]), 8)
        self.assertAlmostEqual(float(self.turns[0][0][0]), 4000 / 32768)
        self.assertAlmostEqual(float(self.turns[0][0][-1]), 8000 / 32768)
        time.sleep(0.15)
        self.assertEqual(len(self.turns), 1)

    def test_empty_audio_duplicate_events_and_capture_failure_are_safe(self):
        self.assertTrue(self.manager.begin_capture())
        self.assertFalse(self.manager.begin_capture())
        self.assertTrue(self.manager.end_capture())
        self.assertFalse(self.manager.end_capture())
        eventually(lambda: len(self.turns) == 1)
        self.assertEqual(len(self.turns[0][0]), 0)

    def test_microphone_failure_reports_and_stops_capture(self):
        errors = []

        def fail_to_open(**_kwargs):
            raise PermissionError("microphone permission denied")

        failed = PushToTalkTurnManager(
            fail_to_open, self.echo, lambda *_: None,
            on_capture_error=errors.append, grace_s=0.01)
        failed.start()
        eventually(lambda: errors)
        self.assertIsInstance(errors[0], PermissionError)
        failed.stop()

    def test_hold_during_busy_response_reopens_capture_without_old_audio(self):
        token = self.manager.begin_response()
        eventually(lambda: self.streams[0].closed)
        self.assertTrue(self.manager.begin_capture())
        eventually(lambda: len(self.streams) == 2 and self.streams[-1].active)
        self.manager.finish_response(token, delay=0, preserve_capture=True)
        self.streams[-1].feed(np.full(4, 6000, dtype=np.int16))
        self.assertTrue(self.manager.end_capture())
        eventually(lambda: len(self.turns) == 1)
        self.assertEqual(len(self.turns[0][0]), 4)


class EmptyRuntimeTurnTests(unittest.TestCase):
    def test_empty_audio_never_reaches_transcription(self):
        from session.runtime import RuntimeController

        runtime = RuntimeController.__new__(RuntimeController)
        runtime.args = SimpleNamespace(ui=True)
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        runtime.session_log = SessionLog(Path(temp.name) / "sessions")
        runtime._capture_turn_id = None
        runtime.turn_manager = Mock()
        runtime.bridge = Mock()
        runtime.on_handsfree_speech(np.zeros(0, dtype=np.float32), 31)
        runtime.turn_manager.finish_response.assert_called_once_with(31)
        runtime.bridge.emit_state.assert_called_once_with("idle")
        records, warning = SessionLog.read_records(runtime.session_log.path)
        self.assertIsNone(warning)
        self.assertEqual(records[0]["details"]["outcome"], "empty_capture")

    def test_hold_interrupts_response_and_prevents_stale_playback(self):
        from session.runtime import RuntimeController
        from session.turn_worker import SerializedTurnWorker

        runtime = RuntimeController.__new__(RuntimeController)
        runtime.turn_manager = Mock(response_lock=threading.RLock())
        runtime.worker = SerializedTurnWorker()
        runtime._active_response_token = 41
        entered, release = threading.Event(), threading.Event()
        stale_playback = []

        def old_response(seq):
            entered.set()
            release.wait(1)
            if not runtime.worker.is_superseded(seq):
                stale_playback.append("played")

        try:
            runtime.worker.submit(old_response)
            self.assertTrue(entered.wait(1))
            with patch("session.runtime.voice.stop_playback", create=True) as stop_playback:
                runtime.interrupt_response_for_capture()
            stop_playback.assert_called_once_with()
            runtime.turn_manager.finish_response.assert_called_once_with(
                41, delay=0, preserve_capture=True)
            release.set()
            runtime.worker.q.join()
            self.assertEqual(stale_playback, [])
            self.assertIsNone(runtime._active_response_token)
        finally:
            release.set()
            runtime.worker.stop()


if __name__ == "__main__":
    unittest.main()
