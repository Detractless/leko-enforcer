"""Run the actual speak function with fake synthesis and playback, without device imports."""
import ast
from pathlib import Path
import sys
import time
import unittest
from typing import Callable, Optional
from unittest.mock import Mock


class SpeechCleanupTests(unittest.TestCase):
    def setUp(self):
        source = Path(__file__).resolve().parents[1] / 'voice' / 'assistant.py'
        tree = ast.parse(source.read_text())
        function = next(node for node in tree.body if isinstance(node, ast.FunctionDef)
                        and node.name == 'speak')
        self.tts = Mock(sample_rate=16000)
        self.tts.synth.return_value = [0] * 100
        self.player = Mock()
        self.player.play.return_value = True
        self.state = Mock()
        self.stop = Mock()
        self.sd = Mock()
        self.sd.get_stream.return_value = None
        self.namespace = dict(Optional=Optional, Callable=Callable, time=time, sys=sys,
                              verbalize_for_speech=lambda text: text, get_tts=lambda: self.tts,
                              VOICE_NAME='test', _echo_audio=Mock(available=True),
                              _output_player=self.player, set_echo_cancellation_enabled=Mock(),
                              set_playback_state=self.state, stop_playback=self.stop, sd=self.sd)
        exec(compile(ast.Module(body=[function], type_ignores=[]), str(source), 'exec'), self.namespace)
        self.speak = self.namespace['speak']

    def test_synthesis_error_stops_output_and_clears_playback(self):
        self.tts.synth.side_effect = RuntimeError('expected synthesis failure')
        self.speak('hello')
        self.stop.assert_called_once()
        self.player.play.assert_not_called()
        self.state.assert_called_with(False, post_mute_s=0.0)

    def test_cancel_during_synthesis_never_starts_playback(self):
        cancelled = False
        def synth(*args, **kwargs):
            nonlocal cancelled
            cancelled = True
            return [0] * 100
        self.tts.synth.side_effect = synth
        self.speak('hello', lambda: cancelled)
        self.player.play.assert_not_called()
        self.sd.play.assert_not_called()
        self.state.assert_called_with(False, post_mute_s=0.0)

    def test_output_error_stops_and_clears_playback(self):
        self.player.play.side_effect = RuntimeError('expected playback failure')
        self.speak('hello')
        self.stop.assert_called_once()
        self.state.assert_called_with(False, post_mute_s=0.0)

    def test_fallback_completion_has_no_spurious_error(self):
        self.player.play.return_value = False
        self.speak('hello')
        self.sd.play.assert_called_once()
        self.stop.assert_not_called()
        self.state.assert_called_with(False, post_mute_s=0.0)


if __name__ == '__main__':
    unittest.main()
