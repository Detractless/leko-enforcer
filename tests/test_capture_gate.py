"""No devices or models: exercise real gate and worker with delayed callbacks."""
import sys
import threading
import time
import unittest
from pathlib import Path
from unittest.mock import Mock

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "voice"))
from audio_io.capture_gate import CaptureGate
from session.turn_worker import SerializedTurnWorker
from turn_detector import TurnDetector


def eventually(predicate):
    deadline = time.monotonic() + 2
    while not predicate():
        if time.monotonic() >= deadline:
            raise AssertionError("Timed out waiting for lifecycle transition")
        time.sleep(0.002)


class FakeStream:
    def __init__(self, callback, **kwargs):
        self.callback = callback
        self.active = False
        self.closed = False
        self.operations = []

    def start(self):
        self.operations.append(("start", threading.current_thread().name))
        self.active = True

    def stop(self):
        self.operations.append(("stop", threading.current_thread().name))
        self.active = False

    def close(self):
        self.operations.append(("close", threading.current_thread().name))
        self.closed = True

    def feed(self, samples):
        self.callback(np.asarray(samples, dtype=np.int16).tobytes(), len(samples), None, None)


class CaptureTests(unittest.TestCase):
    def setUp(self):
        self.streams = []
        self.turns = []
        self.onsets = []
        self.echo = Mock(available=True)
        self.echo.clean_capture.side_effect = lambda x: x
        detector = TurnDetector(model_path=Path('/nonexistent'), speech_detector=False,
                                on_speech_onset=lambda: self.onsets.append(True))
        def factory(**kwargs):
            stream = FakeStream(**kwargs)
            self.streams.append(stream)
            return stream
        self.gate = CaptureGate(factory, detector, self.echo,
                                lambda audio, token: self.turns.append((audio, token)))
        self.worker = SerializedTurnWorker()
        self.gate.start()
        eventually(lambda: self.streams and self.streams[-1].active)
        self.addCleanup(self.worker.stop)
        self.addCleanup(self.gate.stop)

    def test_complete_cycle_fences_before_submission_and_clears_preroll(self):
        old = self.streams[-1]
        old.feed(np.zeros(2560))
        self.assertTrue(self.gate.detector.preroll)
        old.feed(np.full(2560, 5000))
        for _ in range(8):
            old.feed(np.zeros(2560))
        self.assertEqual(len(self.turns), 1)
        token = self.turns[0][1]
        self.assertTrue(self.gate.busy)
        self.assertIsNone(self.gate.begin_response())
        self.assertFalse(self.gate.detector.preroll)
        self.assertFalse(self.gate._raw_buf)
        eventually(lambda: old.closed)
        calls = self.echo.clean_capture.call_count
        for phase in ('transcription', 'thinking', 'synthesis', 'playback'):
            with self.subTest(phase=phase):
                old.feed(np.full(5120, 15000))
                self.assertEqual(self.echo.clean_capture.call_count, calls)
                self.assertEqual(len(self.onsets), 1)
                self.assertEqual(len(self.streams), 1)
        self.gate.finish_response(token, delay=0)
        eventually(lambda: len(self.streams) == 2 and self.streams[-1].active)
        old.feed(np.full(2560, 15000))
        self.assertEqual(self.echo.clean_capture.call_count, calls)
        self.assertFalse(self.gate.detector.preroll)
        self.streams[-1].feed(np.full(2560, 5000))
        self.assertEqual(len(self.onsets), 2)
        self.assertEqual(len(self.gate.detector.buf), 1)
        self.assertTrue(all(thread == 'capture-control' for _, thread in old.operations))

    def test_cancel_slow_worker_then_manual_mute_and_stale_completion(self):
        token = self.gate.begin_response()
        entered, release = threading.Event(), threading.Event()
        self.addCleanup(release.set)
        def run(seq):
            self.gate.wait_paused()
            entered.set()
            release.wait(2)
            self.assertTrue(self.worker.is_superseded(seq))
        self.worker.submit(run, on_done=lambda: self.gate.finish_response(token, 0))
        self.assertTrue(entered.wait(2))
        self.worker.cancel_current()
        self.gate.set_muted(True)
        self.assertTrue(self.gate.busy)
        self.assertIsNone(self.gate.begin_response())
        release.set()
        self.worker.q.join()
        self.assertFalse(self.gate.busy)
        self.assertEqual(len(self.streams), 1)
        self.gate.set_muted(False)
        eventually(lambda: len(self.streams) == 2)
        next_token = self.gate.begin_response()
        self.gate.finish_response(token, 0)
        self.assertTrue(self.gate.busy)
        self.gate.finish_response(next_token, 0)

    def test_errors_release_only_after_actual_capture_stop(self):
        token = self.gate.begin_response()
        def fail(seq):
            self.gate.wait_paused()
            self.assertTrue(self.streams[0].closed)
            raise RuntimeError('simulated transcription/model/TTS error')
        self.worker.submit(fail, on_done=lambda: self.gate.finish_response(token, 0))
        self.worker.q.join()
        eventually(lambda: len(self.streams) == 2)
        self.assertFalse(self.gate.busy)

    def test_cancel_before_worker_starts_still_releases_gate(self):
        entered, release = threading.Event(), threading.Event()
        self.addCleanup(release.set)
        self.worker.submit(lambda seq: (entered.set(), release.wait(2)))
        self.assertTrue(entered.wait(2))
        token = self.gate.begin_response()
        task = Mock()
        self.worker.submit(task, on_done=lambda: self.gate.finish_response(token, 0))
        self.worker.cancel_current()
        release.set()
        self.worker.q.join()
        task.assert_not_called()
        eventually(lambda: len(self.streams) == 2)

    def test_transcription_waits_for_slow_device_stop(self):
        stopping, release, ran = threading.Event(), threading.Event(), threading.Event()
        self.addCleanup(release.set)
        original_stop = self.streams[0].stop
        def slow_stop():
            stopping.set()
            release.wait(2)
            original_stop()
        self.streams[0].stop = slow_stop
        token = self.gate.begin_response()
        self.worker.submit(lambda seq: (self.gate.wait_paused(), ran.set()),
                           on_done=lambda: self.gate.finish_response(token, 0))
        self.assertTrue(stopping.wait(2))
        self.assertFalse(ran.is_set())
        release.set()
        self.worker.q.join()
        self.assertTrue(ran.is_set())

    def test_shutdown_during_busy_never_reopens(self):
        token = self.gate.begin_response()
        self.gate.stop()
        self.gate.finish_response(token, 0)
        self.gate.set_muted(False)
        self.gate.start()
        self.assertIsNone(self.gate.begin_response())
        self.assertTrue(self.streams[0].closed)
        self.assertEqual(len(self.streams), 1)
        self.assertFalse(self.gate._thread.is_alive())
        self.worker.stop()
        with self.assertRaises(RuntimeError):
            self.worker.submit(Mock())

    def test_mic_armed_fires_once_when_capture_resumes(self):
        armed = []
        self.gate.on_armed = lambda: armed.append(True)
        old = self.streams[-1]
        token = self.gate.begin_response()
        old.feed(np.zeros(2560))
        self.assertFalse(armed)
        self.gate.finish_response(token)
        eventually(lambda: len(self.streams) == 2 and self.streams[-1].active)
        self.streams[-1].feed(np.zeros(2560))
        eventually(lambda: armed)
        self.streams[-1].feed(np.zeros(2560))
        self.assertEqual(len(armed), 1)

    def test_muting_idle_closes_device_and_discards_partial_audio(self):
        old = self.streams[0]
        old.feed(np.ones(1280))
        self.assertTrue(self.gate._raw_buf)
        self.gate.set_muted(True)
        eventually(lambda: old.closed)
        self.assertFalse(self.gate._raw_buf)
        self.gate.set_muted(False)
        eventually(lambda: len(self.streams) == 2)
        old.feed(np.full(2560, 5000))
        self.assertFalse(self.onsets)


if __name__ == '__main__':
    unittest.main()
