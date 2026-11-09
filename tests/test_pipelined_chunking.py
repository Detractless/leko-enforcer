"""Tests for pipelined chunking (AC-004, AC-005, AC-006).

Evidence: "intermediate results during recording"
Evidence: "final transcript matches batch"
Evidence: "cancel resets pipeline"
"""
import unittest
from unittest.mock import Mock

from voice.pipelined_chunker import PipelinedChunker


class TestPipelinedChunking(unittest.TestCase):
    def _make_chunker(self, window_s=0.1, sr=16000, transcribe=None):
        if transcribe is None:
            transcribe = lambda samples: f"chunk_{len(samples)}"
        return PipelinedChunker(transcribe, chunk_window_s=window_s, sample_rate=sr)

    def test_intermediate_results_during_recording(self):
        intermediate = []
        transcribe = Mock(side_effect=lambda s: "text")
        chunker = PipelinedChunker(transcribe, chunk_window_s=0.1, sample_rate=16000,
                                   on_intermediate=intermediate.append)
        chunk_size = int(0.1 * 16000)
        # Feed two full windows
        chunker.feed([0.0] * chunk_size)
        self.assertEqual(len(intermediate), 1)
        chunker.feed([0.0] * chunk_size)
        self.assertEqual(len(intermediate), 2)

    def test_final_transcript_matches_batch(self):
        def transcribe(samples):
            return "hello" if len(samples) >= 100 else "hi"
        chunker = PipelinedChunker(transcribe, chunk_window_s=0.1, sample_rate=1000)
        chunk_size = int(0.1 * 1000)
        chunker.feed([0.0] * chunk_size)
        chunker.feed([0.0] * 50)
        result = chunker.finalize()
        self.assertIn("hello", result)
        self.assertIn("hi", result)

    def test_empty_audio_returns_empty(self):
        transcribe = Mock(return_value="x")
        chunker = PipelinedChunker(transcribe, chunk_window_s=1.0, sample_rate=16000)
        result = chunker.finalize()
        self.assertEqual(result, "")
        transcribe.assert_not_called()

    def test_cancel_resets_pipeline(self):
        transcribe = Mock(return_value="text")
        chunker = PipelinedChunker(transcribe, chunk_window_s=0.1, sample_rate=16000)
        chunk_size = int(0.1 * 16000)
        chunker.feed([0.0] * chunk_size)
        chunker.cancel()
        self.assertEqual(chunker.partial_results, [])
        result = chunker.finalize()
        self.assertEqual(result, "")

    def test_short_recording_single_batch(self):
        transcribe = Mock(return_value="short")
        chunker = PipelinedChunker(transcribe, chunk_window_s=30.0, sample_rate=16000)
        chunker.feed([0.0] * 1000)
        result = chunker.finalize()
        self.assertEqual(result, "short")
        transcribe.assert_called_once()