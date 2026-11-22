"""Exercise real runtime callbacks with fake audio, worker, model and CLI."""
import importlib
import sys
import types
import unittest
from pathlib import Path
from unittest.mock import MagicMock, patch
import test_conversation as fixtures
from session.session_log import SessionLog


def _submit_inline(fn, on_done=None):
    try:
        fn(1)
    finally:
        if on_done:
            on_done()


def _capture_manager(callbacks, manager, **kwargs):
    callbacks.update(kwargs)
    return manager


class RuntimeConfirmationTests(unittest.TestCase):
    setUp = fixtures.ConversationTests.setUp
    query = fixtures.ConversationTests.query
    run_tool = fixtures.ConversationTests.run_tool
    writes = fixtures.ConversationTests.writes

    def _drive_ui_scenario(self, callbacks, voice):
        ui = callbacks["ui"]
        ui({"action": "query", "text": "block reddit"})
        first_id = self.state.pending["id"]
        voice.transcribe_audio.return_value = "yep"
        callbacks["on_turn_complete"]([0.1], 1)
        self.assertEqual(len(self.writes()), 1)
        ui({"action": "confirm", "draft": {"id": first_id}})
        self.assertEqual(len(self.writes()), 1)
        before = self.model.call_count
        voice.transcribe_audio.return_value = "um"
        callbacks["on_turn_complete"]([0.1], 1)
        self.assertEqual(self.model.call_count, before)
        for action in ("cancel", "stop", "mute"):
            ui({"action": "query", "text": "block reddit"})
            draft = {"id": self.state.pending["id"], "intent": {"tool": "start_session"}}
            ui({"action": action, "draft": draft})
            ui({"action": "confirm", "draft": draft})
            self.assertIsNone(self.state.pending)
        self.assertEqual(len(self.writes()), 1)
        ui({"action": "query", "text": "block reddit"})
        draft = {"id": self.state.pending["id"], "intent": {"tool": "start_session"}}
        ui({"action": "confirm", "draft": draft})
        self.assertEqual(len(self.writes()), 2)
        self.assertIn("add-block", self.writes()[-1])
        return "exit"

    def test_changed_request_cannot_use_stale_confirmation(self):
        self.query("Block reddit for 30 minutes")
        stale_id = self.state.pending["id"]
        self.intent["arguments"]["duration"] = "45m"
        self.query("Actually make that 45 minutes")
        current_id = self.state.pending["id"]
        self.assertNotEqual(current_id, stale_id)
        self.query("yes", confirmation_id=stale_id)
        self.assertEqual(len(self.writes()), 0)
        self.assertEqual(self.state.pending["id"], current_id)
        self.query("yes", confirmation_id=current_id)
        self.assertEqual(len(self.writes()), 1)
        self.assertEqual(self.writes()[0][self.writes()[0].index("--for") + 1], "45m")

    def test_spoken_filler_order_and_ui_cancel_stop_mute(self):
        voice = fixtures.fake_voice
        worker = MagicMock()
        worker.is_superseded.return_value = False
        worker.submit.side_effect = _submit_inline
        manager = MagicMock()
        manager.detector.speech_detector = None
        manager.busy = False
        callbacks = {}
        manager_factory = lambda **kwargs: _capture_manager(callbacks, manager, **kwargs)
        self.bridge.start_ui_process = lambda on_action: callbacks.update(ui=on_action)
        self.bridge.close = lambda: None
        with patch.dict(sys.modules, {"numpy": types.SimpleNamespace(ndarray=object)}):
            runtime = importlib.import_module("session.runtime")
        drive = lambda *_: self._drive_ui_scenario(callbacks, voice)
        with patch.multiple(voice, create=True,
                REPO_ROOT=fixtures.Path("/fake"), SerializedTurnWorker=lambda: worker,
                HandsfreeTurnManager=manager_factory, PushToTalkTurnManager=manager_factory,
                acquire_single_instance_lock=lambda: None,
                ensure_server=lambda: None, _echo_audio=types.SimpleNamespace(available=False),
                _is_muted=False, _is_speaking=False, _playback_active=False, stop_playback=lambda: None,
                speak=lambda *a, **kw: None, transcribe_audio=MagicMock(),
                process_query=fixtures.process_query), patch.object(
                    runtime, "prepare_settings", return_value="/fake/settings.zip"), patch.object(
                    runtime, "SessionLog", side_effect=lambda: SessionLog(Path(self.tmp.name) / "sessions")), patch.object(
                    runtime, "Conversation", return_value=self.state), patch.object(
                    runtime, "UIPipelineBridge", return_value=self.bridge), patch.object(
                    runtime.time, "sleep"), patch("builtins.input", side_effect=drive), patch.object(
                    sys, "argv", ["assistant", "--ui", "--speech-detector", "loudness"]):
            runtime.main()


if __name__ == "__main__":
    unittest.main()
