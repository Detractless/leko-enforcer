"""Tests for quiet-speech normalization, idle AEC bypass, and Alkove isolation."""
import contextlib
import io
import tempfile
import unittest
from pathlib import Path

import numpy as np

from voice.audio_io.echo_canceller import EchoAudioProcessor
from voice.core.alkove_isolator import (
    AlkoveIsolator,
    IdleSafeEchoAudio,
    MIN_SPEECH_RMS,
    max_chunk_rms,
    normalize_quiet_speech,
    prepare_audio_for_stt,
)
from voice.session.enroll_voice import enroll_from_pcm


class AlkoveAndPickupTests(unittest.TestCase):
    def test_quiet_speech_normalization_drops_silence_and_boosts_quiet_voice(self):
        silence = np.full(4800, 0.0005, dtype=np.float32)
        self.assertIsNone(normalize_quiet_speech(silence))

        quiet = np.sin(np.linspace(0, 40 * np.pi, 4800, dtype=np.float32)) * 0.004
        boosted = normalize_quiet_speech(quiet)
        self.assertIsNotNone(boosted)
        self.assertGreater(max_chunk_rms(boosted), 0.03)
        self.assertLessEqual(float(np.max(np.abs(boosted))), 0.95)

    def test_prepare_audio_falls_back_gracefully_when_isolator_disabled(self):
        with tempfile.TemporaryDirectory() as tmp:
            settings = Path(tmp) / "leko-settings.json"
            settings.write_text('{"speaker_isolation": false}', encoding="utf-8")
            isolator = AlkoveIsolator(voiceprint_path=Path(tmp) / "vp.json", settings_path=settings)
            quiet = np.sin(np.linspace(0, 50 * np.pi, 6400, dtype=np.float32)) * 0.0035
            prepared = prepare_audio_for_stt(quiet, isolator=isolator)
            self.assertIsNotNone(prepared)
            self.assertGreaterEqual(max_chunk_rms(prepared), MIN_SPEECH_RMS)

    def test_idle_aec_wrapper_passes_capture_through_when_playback_idle(self):
        inner = EchoAudioProcessor()
        wrapped = IdleSafeEchoAudio(inner)
        if not wrapped.available:
            self.skipTest("Native AEC dylib not built")
        tone = (np.sin(np.linspace(0, 20 * np.pi, 1600)) * 500).astype(np.int16)
        cleaned = wrapped.clean_capture(tone)
        self.assertIsNotNone(cleaned)
        np.testing.assert_array_equal(cleaned, tone)

    def test_alkove_real_models_and_enrollment(self):
        try:
            import alkove  # noqa: F401
        except ImportError:
            self.skipTest("alkove package not installed")
        with tempfile.TemporaryDirectory() as tmp:
            vp_path = Path(tmp) / "voiceprint.json"
            settings = Path(tmp) / "leko-settings.json"
            speech = np.sin(np.linspace(0, 180 * np.pi, 28_000, dtype=np.float32)) * 0.04
            enroll_from_pcm(speech, out_path=vp_path)
            self.assertTrue(vp_path.exists())
            isolator = AlkoveIsolator(voiceprint_path=vp_path, settings_path=settings)
            out, used = isolator.isolate(speech)
            self.assertTrue(used)
            self.assertEqual(len(out), len(speech))

    def test_prepare_audio_logs_pre_and_post_rms_for_kept_and_stripped_turns(self):
        class _StubIsolator:
            def __init__(self, out_amp: float, using_voiceprint: bool):
                self._out_amp = out_amp
                self.using_voiceprint = using_voiceprint

            def isolate(self, audio_f32, sample_rate=16000):
                return np.full_like(audio_f32, self._out_amp), True

        speech = np.sin(np.linspace(0, 60 * np.pi, 6400, dtype=np.float32)) * 0.01
        buf = io.StringIO()
        with contextlib.redirect_stdout(buf):
            kept = prepare_audio_for_stt(speech, isolator=_StubIsolator(0.04, True))
            stripped = prepare_audio_for_stt(speech, isolator=_StubIsolator(0.0001, True))
        logs = buf.getvalue()
        self.assertIsNotNone(kept)
        self.assertIsNone(stripped)
        self.assertIn("[Alkove Filter] kept speech (mode=voiceprint", logs)
        self.assertIn("[Alkove Filter] stripped non-matching audio (mode=voiceprint", logs)
        self.assertIn("pre_rms=", logs)
        self.assertIn("post_rms=", logs)


if __name__ == "__main__":
    unittest.main()
