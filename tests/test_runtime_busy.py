"""Real runtime, capture gate, worker and query flow; all devices/models are fakes."""
import importlib
import json
import sys
import threading
import types
import unittest
from pathlib import Path
from unittest.mock import Mock, patch

import test_conversation as fixtures
from test_capture_gate import FakeStream, eventually
from audio_io.capture_gate import CaptureGate
from session.turn_worker import SerializedTurnWorker
from turn_detector import TurnDetector


class RuntimeBusyTests(unittest.TestCase):
    setUp = fixtures.ConversationTests.setUp
    run_tool = fixtures.ConversationTests.run_tool

    def run_runtime(self, drive, transcribe, query):
        runtime = importlib.import_module('session.runtime')
        self.callbacks, self.streams, self.failures = {}, [], []
        self.worker = SerializedTurnWorker()
        echo = Mock(available=False)
        def factory(**kwargs):
            stream = FakeStream(**kwargs)
            self.streams.append(stream)
            return stream
        def manager(**kwargs):
            self.callbacks.update(kwargs)
            detector = TurnDetector(model_path=Path('/nonexistent'), speech_detector=False,
                                    on_speech_onset=kwargs['on_speech_onset'])
            self.manager = CaptureGate(factory, detector, echo, kwargs['on_turn_complete'])
            return self.manager
        def console(*_):
            try:
                if not self.streams:
                    eventually(lambda: self.streams and self.streams[-1].active)
                return drive() or "exit"
            except BaseException as exc:
                self.failures.append(exc)
            return 'exit'
        self.bridge.start_ui_process = lambda on_action: self.callbacks.update(ui=on_action)
        self.bridge.close = lambda: None
        stt_lifecycle = Mock()
        with patch.multiple(fixtures.fake_voice, create=True,
                REPO_ROOT=Path('/fake'), SerializedTurnWorker=lambda: self.worker,
                HandsfreeTurnManager=manager, acquire_single_instance_lock=lambda: None,
                ensure_server=lambda: None, _echo_audio=echo, _is_muted=False,
                stop_playback=Mock(), transcribe_audio=transcribe,
                _stt_lifecycle=stt_lifecycle, get_stt=lambda: None,
                process_query=query), patch.object(runtime, 'Conversation', return_value=self.state), \
                patch.object(runtime, 'UIPipelineBridge', return_value=self.bridge), \
                patch('builtins.input', side_effect=console), \
                patch.object(sys, 'argv', ['assistant', '--ui', '--speech-detector', 'loudness']):
            runtime.main()
            self.worker.thread.join(2)
        if self.failures:
            raise self.failures[0]
        self.assertFalse(self.worker.thread.is_alive())
        self.assertTrue(all(stream.closed for stream in self.streams))

    def check_busy(self):
        self.assertTrue(self.manager.busy)
        self.assertTrue(all(stream.closed for stream in self.streams))
        self.assertIsNone(self.manager.begin_response())
        before = self.worker.seq
        self.callbacks['ui']({'action': 'query', 'text': 'must not queue'})
        self.assertEqual(self.worker.seq, before)
        events = len(self.events)
        self.callbacks['on_speech_onset']()
        self.assertEqual(len(self.events), events)
        self.streams[0].feed([15000] * 2560)
        self.assertFalse(self.manager.detector.buf)
        self.assertFalse(self.manager.detector.preroll)

    def test_unfinished_speech_stays_closed_through_real_query_response(self):
        stages = []
        def stt(audio):
            self.check_busy()
            stages.append('transcription')
            return 'I think I need'
        def model(*args, **kwargs):
            self.check_busy()
            stages.append('thinking')
            return {'action': 'reply', 'content': 'What do you need?'}
        def speak(text, cancel):
            self.check_busy()
            stages.append('synthesis')
            self.check_busy()
            stages.append('playback')
        def query(*args, **kwargs):
            return fixtures.process_query(*args, **kwargs, speak_fn=speak)
        self.model.side_effect = model
        def drive():
            token = self.manager.begin_response()
            self.callbacks['on_turn_complete'](None, token)
            self.worker.q.join()
            self.assertEqual(stages, ['transcription', 'thinking', 'synthesis', 'playback'])
            eventually(lambda: len(self.streams) == 2 and self.streams[-1].active)
            self.assertFalse(self.manager.busy)
        with patch('builtins.print') as output:
            self.run_runtime(drive, stt, query)
        audio_lines = [call.args[0] for call in output.call_args_list
                       if call.args and str(call.args[0]).startswith('[Audio turn] ')]
        stt_lines = [call.args[0] for call in output.call_args_list
                     if call.args and str(call.args[0]).startswith('[STT] elapsed=')]
        transcript_lines = [call.args[0] for call in output.call_args_list
                            if call.args and str(call.args[0]).lstrip().startswith('[Transcribed]')]
        self.assertEqual(len(audio_lines), 1)
        self.assertEqual(json.loads(audio_lines[0][len('[Audio turn] '):])['audio_seconds'], 0.0)
        self.assertEqual(len(stt_lines), 1)
        self.assertRegex(stt_lines[0], r'\[STT\] elapsed=\d+\.\d+s result=text')
        self.assertEqual(transcript_lines, ['\n[Transcribed]: "I think I need"'])

    def test_ui_cancel_mute_and_shutdown_during_synthesis(self):
        entered, release = threading.Event(), threading.Event()
        self.addCleanup(release.set)
        calls = []
        def query(text, *args, **kwargs):
            calls.append(text)
            self.check_busy()
            entered.set()
            release.wait(2)
            self.assertTrue(kwargs['is_cancelled']())
        def drive():
            self.callbacks['ui']({'action': 'query', 'text': 'typed request'})
            self.assertTrue(entered.wait(2))
            self.callbacks['ui']({'action': 'stop'})
            self.callbacks['ui']({'action': 'mute', 'muted': True})
            self.assertTrue(self.manager.busy)
            self.callbacks['ui']({'action': 'listen_start'})
            self.assertTrue(self.manager.busy)
            self.callbacks['ui']({'action': 'window_closed'})
            release.set()
            self.worker.q.join()
            self.assertEqual(calls, ['typed request'])
            self.assertEqual(len(self.streams), 1)
        self.run_runtime(drive, Mock(), query)

    def test_ignored_filler_transcript_is_logged_before_guard_drops_it(self):
        query = Mock()
        def drive():
            token = self.manager.begin_response()
            self.callbacks['on_turn_complete'](None, token)
            self.worker.q.join()
            query.assert_not_called()
        with patch('builtins.print') as output:
            self.run_runtime(drive, lambda _: 'um', query)
        lines = [str(call.args[0]) for call in output.call_args_list if call.args]
        self.assertIn('\n[Transcribed]: "um"', lines)
        self.assertIn('[Turn Guard]: Dropped an isolated filler transcript.', lines)

    def test_console_query_uses_same_gate(self):
        entered, release = threading.Event(), threading.Event()
        self.addCleanup(release.set)
        calls = []
        reads = 0
        def query(text, *args, **kwargs):
            calls.append(text)
            self.check_busy()
            entered.set()
            release.wait(2)
        def drive():
            nonlocal reads
            reads += 1
            if reads == 1:
                return 'console request'
            self.assertTrue(entered.wait(2))
            self.assertTrue(self.manager.busy)
            release.set()
            self.worker.q.join()
            self.assertEqual(calls, ['console request'])
            return 'exit'
        self.run_runtime(drive, Mock(), query)

    def test_transcription_error_recovers_without_processing_buffered_audio(self):
        def stt(audio):
            self.check_busy()
            raise RuntimeError('expected STT failure')
        query = Mock()
        def drive():
            token = self.manager.begin_response()
            self.callbacks['on_turn_complete'](None, token)
            self.worker.q.join()
            eventually(lambda: len(self.streams) == 2 and self.streams[-1].active)
            query.assert_not_called()
            self.assertFalse(self.manager._raw_buf)
        self.run_runtime(drive, stt, query)


if __name__ == '__main__':
    unittest.main()
