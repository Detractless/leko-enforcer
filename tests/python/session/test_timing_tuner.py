"""Tests for localhost timing tuner server, WAV synthesis analysis, and saved timing config."""
import base64
import json
import tempfile
import threading
import unittest
import urllib.request
from pathlib import Path
from unittest.mock import patch

import numpy as np

from voice.core.timing_tuner import TunerState, create_server, save_card_timing
from voice.core.ui_bridge import compute_word_timings_ms, load_card_timing


class TimingTunerTests(unittest.TestCase):
    def test_saved_timing_config_drives_compute_word_timings_ms(self):
        with tempfile.TemporaryDirectory() as tmp:
            cfg_path = Path(tmp) / "card-timing.json"
            saved = save_card_timing({
                "reveal_mode": "letter",
                "lead_ms": -40,
                "energy_threshold": 0.05,
                "gap_bridge_ms": 100,
                "end_pad_ms": -20,
            }, path=cfg_path)
            self.assertEqual(saved["reveal_mode"], "letter")
            self.assertEqual(saved["lead_ms"], -40)
            loaded = load_card_timing(cfg_path)
            self.assertEqual(loaded, saved)

            sr = 24000
            pcm = np.zeros(int(sr * 1.4), dtype=np.float32)
            pcm[int(sr * 0.16):int(sr * 1.16)] = 0.25
            with patch("voice.core.ui_bridge.CARD_TIMING_PATH", cfg_path):
                details = compute_word_timings_ms(pcm, sr, "No active countdowns.", return_details=True)
            self.assertEqual(details["segments_ms"], [(160, 1160)])
            self.assertEqual(details["timings_ms"][0], 120)
            self.assertGreater(details["timings_ms"][1], details["timings_ms"][0])
            self.assertGreater(details["timings_ms"][2], details["timings_ms"][1])

    def test_http_endpoints_serve_ui_synth_wav_and_persist_config(self):
        sr = 24000
        pcm = np.zeros(int(sr * 1.2), dtype=np.float32)
        pcm[int(sr * 0.1):int(sr * 0.9)] = 0.2
        calls = []

        def fake_synth(spoken):
            calls.append(spoken)
            return pcm, sr, "fake"

        with tempfile.TemporaryDirectory() as tmp:
            cfg_path = Path(tmp) / "card-timing.json"
            state = TunerState(synth_fn=fake_synth, config_path=cfg_path)
            server = create_server("127.0.0.1", 0, state=state, config_path=cfg_path)
            port = server.server_address[1]
            thread = threading.Thread(target=server.serve_forever, daemon=True)
            thread.start()
            try:
                with urllib.request.urlopen(f"http://127.0.0.1:{port}/") as resp:
                    self.assertEqual(resp.status, 200)
                    self.assertIn(b"Leko Speech", resp.read())

                with urllib.request.urlopen(f"http://127.0.0.1:{port}/api/config") as resp:
                    cfg_data = json.loads(resp.read().decode("utf-8"))
                self.assertIn("No active countdowns.", cfg_data["presets"])

                req = urllib.request.Request(
                    f"http://127.0.0.1:{port}/api/synth",
                    data=json.dumps({"text": "No active countdowns.", "lead_ms": -20}).encode("utf-8"),
                    headers={"Content-Type": "application/json"},
                )
                with urllib.request.urlopen(req) as resp:
                    synth_data = json.loads(resp.read().decode("utf-8"))
                self.assertEqual(synth_data["engine"], "fake")
                self.assertEqual(synth_data["timings_ms"][0], 80)
                wav_bytes = base64.b64decode(synth_data["audio_b64"])
                self.assertTrue(wav_bytes.startswith(b"RIFF"))

                req2 = urllib.request.Request(
                    f"http://127.0.0.1:{port}/api/synth",
                    data=json.dumps({"text": "No active countdowns.", "lead_ms": -60, "include_audio": False}).encode("utf-8"),
                    headers={"Content-Type": "application/json"},
                )
                with urllib.request.urlopen(req2) as resp:
                    recomputed = json.loads(resp.read().decode("utf-8"))
                self.assertNotIn("audio_b64", recomputed)
                self.assertEqual(recomputed["timings_ms"][0], 40)
                self.assertEqual(len(calls), 1)

                save_req = urllib.request.Request(
                    f"http://127.0.0.1:{port}/api/config",
                    data=json.dumps({"reveal_mode": "letter", "lead_ms": -50}).encode("utf-8"),
                    headers={"Content-Type": "application/json"},
                )
                with urllib.request.urlopen(save_req) as resp:
                    saved = json.loads(resp.read().decode("utf-8"))
                self.assertTrue(saved["ok"])
                self.assertEqual(load_card_timing(cfg_path)["reveal_mode"], "letter")
                self.assertEqual(load_card_timing(cfg_path)["lead_ms"], -50)
            finally:
                server.shutdown()
                server.server_close()


if __name__ == "__main__":
    unittest.main()
