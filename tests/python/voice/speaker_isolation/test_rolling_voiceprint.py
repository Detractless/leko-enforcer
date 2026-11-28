"""Tests for rolling 20-turn voiceprint learning, intruder immunity, and locking."""
import json
import tempfile
import unittest
from pathlib import Path

import numpy as np

from voice.core.alkove_isolator import AlkoveIsolator
from voice.session.enroll_voice import enroll_from_pcm
from voice.session.rolling_voiceprint import RollingVoiceprintStore


def _tone(freq_hz: float, seconds: float = 2.0, amp: float = 0.04) -> np.ndarray:
    t = np.linspace(0, seconds, int(16000 * seconds), endpoint=False, dtype=np.float32)
    return (np.sin(2.0 * np.pi * freq_hz * t) * amp).astype(np.float32)


def _fake_embed(audio: np.ndarray) -> dict:
    # Classify synthetic clips cleanly into speaker A (<300Hz) vs speaker B (>=300Hz).
    crossings = int(np.sum((audio[:-1] <= 0) & (audio[1:] > 0)))
    hz = crossings / (len(audio) / 16000.0)
    return {"speaker": "A" if hz < 300.0 else "B", "hz": round(hz, 1)}


def _fake_verify(a: dict, b: dict):
    same = a.get("speaker") == b.get("speaker")
    return same, 0.95 if same else 0.15


class RollingVoiceprintTests(unittest.TestCase):
    def test_promotes_dominant_speaker_and_evicts_past_20_turns(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            hist = root / "voiceprint-history.json"
            vp = root / "voiceprint.json"
            store = RollingVoiceprintStore(
                history_path=hist, voiceprint_path=vp, embed_fn=_fake_embed, verify_fn=_fake_verify
            )
            clip_a = _tone(160.0)
            for _ in range(3):
                self.assertFalse(store.record_turn(clip_a, "hello"))
            self.assertFalse(vp.exists())

            self.assertTrue(store.record_turn(clip_a, "fourth turn"))
            saved = json.loads(vp.read_text(encoding="utf-8"))
            self.assertEqual(saved["source"], "auto")
            self.assertEqual(saved["voiceprint"]["speaker"], "A")

            # Two intruder turns must not override speaker A.
            clip_b = _tone(440.0)
            store.record_turn(clip_b, "guest one")
            store.record_turn(clip_b, "guest two")
            saved_after = json.loads(vp.read_text(encoding="utf-8"))
            self.assertEqual(saved_after["voiceprint"]["speaker"], "A")

            # Sustained switch to speaker B (>10 of 20 turns) evicts old turns and adapts.
            for _ in range(18):
                store.record_turn(clip_b, "new mic")
            hist_data = json.loads(hist.read_text(encoding="utf-8"))
            self.assertEqual(len(hist_data["entries"]), 20)
            adapted = json.loads(vp.read_text(encoding="utf-8"))
            self.assertEqual(adapted["voiceprint"]["speaker"], "B")

    def test_manual_voiceprint_is_not_overwritten_and_short_clips_buffer(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            hist = root / "voiceprint-history.json"
            vp = root / "voiceprint.json"
            vp.write_text(json.dumps({"source": "manual", "voiceprint": {"speaker": "A"}}), encoding="utf-8")
            store = RollingVoiceprintStore(
                history_path=hist, voiceprint_path=vp, embed_fn=_fake_embed, verify_fn=_fake_verify
            )
            short_b = _tone(440.0, seconds=0.9)
            self.assertFalse(store.record_turn(short_b, "yes"))
            self.assertFalse(hist.exists())
            for _ in range(5):
                store.record_turn(short_b, "short buffered")
            self.assertEqual(json.loads(vp.read_text(encoding="utf-8"))["source"], "manual")

    def test_isolator_unwraps_wrapped_manual_or_auto_voiceprint(self):
        try:
            import alkove  # noqa: F401
        except ImportError:
            self.skipTest("alkove not installed")
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            vp_path = root / "voiceprint.json"
            enroll_from_pcm(_tone(180.0, seconds=2.0), out_path=vp_path)
            payload = json.loads(vp_path.read_text(encoding="utf-8"))
            self.assertEqual(payload["source"], "manual")
            isolator = AlkoveIsolator(voiceprint_path=vp_path, settings_path=root / "missing.json")
            out, used = isolator.isolate(_tone(180.0, seconds=1.0))
            self.assertTrue(used)
            self.assertEqual(len(out), 16000)


if __name__ == "__main__":
    unittest.main()
