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

sys.path.insert(0, str(Path(__file__).resolve().parents[3]))
import types
import voice.core
fake_voice = types.ModuleType("voice.core.assistant")
fake_voice.LEKO_BIN = "FAKE-LEKO"
fake_voice.REPO_ROOT = Path(__file__).resolve().parents[3]
fake_voice.tool_to_cli_args = lambda *a, **k: []
fake_voice.verbalize_for_speech = lambda text: text
fake_voice.format_status_summary = lambda output, *_: output
fake_voice.parse_user_intent_safe = lambda *a, **k: {"action": "reply", "content": "Ready."}
fake_voice.parse_request_update_safe = lambda *a, **k: {}
fake_voice.speak = lambda *a, **k: None
fake_voice.stop_playback = lambda: None
fake_voice.SerializedTurnWorker = object
sys.modules["voice.core.assistant"] = fake_voice
sys.modules["voice.core"].assistant = fake_voice
from voice.audio_io.push_to_talk.push_to_talk import PushToTalkDetector, PushToTalkTurnManager
from voice.session.session_log import SessionLog


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
        from voice.session.runtime import RuntimeController

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
        from voice.session.runtime import RuntimeController
        from voice.session.turn_worker import SerializedTurnWorker

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
            with patch("voice.session.runtime.voice.stop_playback", create=True) as stop_playback:
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

    def test_dictation_trigger_does_not_edit_protected_voice_files(self):
        ROOT = Path(__file__).resolve().parents[3]
        import subprocess
        roots = ["src/daemon", "src/blocker-extension", "src/page-regions", "enforcers",
                 "voice/audio_io", "voice/normalization"]
        listed = subprocess.run(
            ["git", "ls-files", *roots], cwd=ROOT, capture_output=True, text=True, check=True,
        ).stdout.splitlines()
        import hashlib
        combined = hashlib.sha256()
        for name in sorted(listed):
            combined.update(name.encode("utf-8"))
            combined.update(b"\0")
            combined.update((ROOT / name).read_bytes())
        self.assertEqual(
            combined.hexdigest(),
            "92aaff9716e775ab77c0f10c332395ecfb81daeff9194df8542a57b4c7728b4e",
        )
        runtime = (ROOT / "voice/session/runtime.py").read_text(encoding="utf-8")
        self.assertLessEqual(len(runtime.splitlines()), 349)
        runtime_text = (ROOT / "voice/session/runtime.py").read_text(encoding="utf-8")
        self.assertIn("ptt_start", runtime_text)
        self.assertIn("ptt_end", runtime_text)

    def test_warm_stt_does_not_emit_idle_while_holding(self):
        from voice.session.runtime import RuntimeController
        controller = object.__new__(RuntimeController)
        controller.args = SimpleNamespace(ui=True)
        controller.shutdown = threading.Event()
        controller.turn_manager = SimpleNamespace(busy=False, capture_mode="held")
        emitted = []
        controller.bridge = SimpleNamespace(emit_state=lambda state, label=None: emitted.append(state))
        controller._warm_stt(1)
        self.assertEqual(emitted, [])
        controller.turn_manager.capture_mode = "idle"
        controller._warm_stt(2)
        self.assertEqual(emitted, ["idle"])

    def test_warm_stt_preloads_tts_alongside_stt(self):
        from voice.session.runtime import RuntimeController
        controller = object.__new__(RuntimeController)
        controller.args = SimpleNamespace(ui=True)
        controller.shutdown = threading.Event()
        controller.turn_manager = SimpleNamespace(busy=False, capture_mode="idle")
        emitted = []
        controller.bridge = SimpleNamespace(emit_state=lambda state, label=None: emitted.append(state))
        with patch("voice.session.runtime.voice.warm_tts", create=True) as warm_tts:
            controller._warm_stt(1)
        warm_tts.assert_called_once_with()
        self.assertEqual(emitted, ["idle"])


class StreamRecoveryTests(unittest.TestCase):
    def test_succeeds_without_refresh_when_first_open_works(self):
        from voice.core.stream_recovery import open_input_stream
        calls = []
        sd = SimpleNamespace(
            _terminate=lambda: calls.append("terminate"),
            _initialize=lambda: calls.append("initialize"),
            RawInputStream=lambda **kw: (calls.append(("open", kw)), "stream")[1],
        )
        stream = open_input_stream(sd, sleep_fn=lambda _s: calls.append("sleep"), samplerate=16000, blocksize=1280)
        self.assertEqual(stream, "stream")
        self.assertEqual(calls, [("open", {"samplerate": 16000, "blocksize": 1280})])

    def test_refreshes_portaudio_and_falls_back_to_auto_blocksize(self):
        from voice.core.stream_recovery import open_input_stream
        events = []

        def open_stream(**kw):
            events.append(("open", kw.get("blocksize")))
            if len([e for e in events if e[0] == "open"]) < 3:
                raise RuntimeError("Error opening RawInputStream: PaErrorCode -9986")
            return "recovered-stream"

        sd = SimpleNamespace(
            _terminate=lambda: events.append("terminate"),
            _initialize=lambda: events.append("initialize"),
            RawInputStream=open_stream,
        )
        stream = open_input_stream(sd, sleep_fn=lambda s: events.append(("sleep", round(s, 2))),
                                   samplerate=16000, blocksize=1280)
        self.assertEqual(stream, "recovered-stream")
        self.assertEqual(events, [
            ("open", 1280), "terminate", "initialize", ("sleep", 0.15),
            ("open", 1280), "terminate", "initialize", ("sleep", 0.30),
            ("open", 0),
        ])

    def test_skips_portaudio_refresh_while_playback_stream_is_active(self):
        from voice.core.stream_recovery import open_input_stream
        events = []

        def open_stream(**kw):
            events.append("open")
            if events.count("open") == 1:
                raise RuntimeError("transient -9986")
            return "stream"

        sd = SimpleNamespace(
            _terminate=lambda: events.append("terminate"),
            _initialize=lambda: events.append("initialize"),
            RawInputStream=open_stream,
        )
        stream = open_input_stream(sd, can_refresh=lambda: False, sleep_fn=lambda _s: None, blocksize=1280)
        self.assertEqual(stream, "stream")
        self.assertEqual(events, ["open", "open"])


if __name__ == "__main__":
    unittest.main()
