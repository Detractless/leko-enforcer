"""Tests for Bluetooth headset mic avoidance during stream recovery."""
import sys
import unittest
from pathlib import Path
from types import SimpleNamespace

sys.path.insert(0, str(Path(__file__).resolve().parents[4]))
from voice.core.stream_recovery import (
    _fourcc,
    is_bluetooth_output,
    open_input_stream,
    pad_bluetooth_preroll,
    resolve_input_device,
    sync_output_device,
)
from voice.core.ui_bridge import compute_word_timings_ms


def _fake_sd(devices, default_pair, on_refresh=None):
    opened = []
    events = []

    def raw_stream(**kw):
        opened.append(kw)
        return "stream"

    def terminate():
        events.append("terminate")

    def initialize():
        events.append("initialize")
        if on_refresh:
            on_refresh()

    return SimpleNamespace(
        default=SimpleNamespace(device=default_pair),
        query_devices=lambda: devices,
        RawInputStream=raw_stream,
        _terminate=terminate,
        _initialize=initialize,
        opened=opened,
        events=events,
    )


class BluetoothInputRoutingTests(unittest.TestCase):
    def test_prefers_builtin_mic_when_bluetooth_headset_shares_input_and_output(self):
        devices = [
            {"index": 0, "name": "MacBook Air Microphone", "max_input_channels": 1, "max_output_channels": 0, "default_samplerate": 48000.0},
            {"index": 1, "name": "MacBook Air Speakers", "max_input_channels": 0, "max_output_channels": 2, "default_samplerate": 48000.0},
            {"index": 2, "name": "Galaxy Buds2 (5B05)", "max_input_channels": 1, "max_output_channels": 0, "default_samplerate": 16000.0},
            {"index": 3, "name": "Galaxy Buds2 (5B05)", "max_input_channels": 0, "max_output_channels": 2, "default_samplerate": 44100.0},
        ]
        sd = _fake_sd(devices, [2, 3])
        self.assertEqual(resolve_input_device(sd), 0)
        open_input_stream(sd, samplerate=16000, channels=1)
        self.assertEqual(sd.opened[-1]["device"], 0)

    def test_keeps_default_input_for_usb_or_builtin_audio(self):
        usb_devices = [
            {"index": 0, "name": "MacBook Air Microphone", "max_input_channels": 1, "max_output_channels": 0, "default_samplerate": 48000.0},
            {"index": 1, "name": "USB Headset", "max_input_channels": 1, "max_output_channels": 0, "default_samplerate": 48000.0},
            {"index": 2, "name": "USB Headset", "max_input_channels": 0, "max_output_channels": 2, "default_samplerate": 48000.0},
        ]
        sd = _fake_sd(usb_devices, [1, 2])
        self.assertIsNone(resolve_input_device(sd))
        open_input_stream(sd, samplerate=16000, channels=1)
        self.assertNotIn("device", sd.opened[-1])

    def test_respects_explicit_device_and_handles_missing_fallback(self):
        bt_only = [
            {"index": 0, "name": "Galaxy Buds2 (5B05)", "max_input_channels": 1, "max_output_channels": 0, "default_samplerate": 16000.0},
            {"index": 1, "name": "Galaxy Buds2 (5B05)", "max_input_channels": 0, "max_output_channels": 2, "default_samplerate": 44100.0},
        ]
        sd = _fake_sd(bt_only, [0, 1])
        self.assertIsNone(resolve_input_device(sd))
        open_input_stream(sd, device=0, samplerate=16000, channels=1)
        self.assertEqual(sd.opened[-1]["device"], 0)


class OutputDeviceSyncTests(unittest.TestCase):
    def test_refreshes_portaudio_when_live_output_differs_from_cached_default(self):
        devices = [
            {"index": 0, "name": "MacBook Air Microphone"},
            {"index": 1, "name": "MacBook Air Speakers"},
        ]
        sd = _fake_sd(devices, [0, 1])
        self.assertTrue(sync_output_device(sd, get_live_output_name=lambda: "Galaxy Buds2 (5B05)"))
        self.assertEqual(sd.events, ["terminate", "initialize"])

    def test_skips_refresh_when_output_matches_or_input_is_active(self):
        devices = [
            {"index": 0, "name": "MacBook Air Microphone"},
            {"index": 1, "name": "Galaxy Buds2 (5B05)"},
        ]
        sd = _fake_sd(devices, [0, 1])
        self.assertFalse(sync_output_device(sd, get_live_output_name=lambda: "Galaxy Buds2 (5B05)"))
        self.assertEqual(sd.events, [])
        self.assertFalse(sync_output_device(sd, can_refresh=lambda: False, get_live_output_name=lambda: "Other"))
        self.assertEqual(sd.events, [])


class BluetoothPrerollTests(unittest.TestCase):
    def test_detects_bluetooth_and_ble_transports_only(self):
        self.assertTrue(is_bluetooth_output(get_transport=lambda: _fourcc("blue")))
        self.assertTrue(is_bluetooth_output(get_transport=lambda: _fourcc("blea")))
        self.assertFalse(is_bluetooth_output(get_transport=lambda: _fourcc("bltn")))
        self.assertFalse(is_bluetooth_output(get_transport=lambda: None))

    def test_pads_pilot_preroll_only_when_bluetooth_needs_warmup(self):
        import numpy as np

        pcm = np.full(2400, 0.25, dtype=np.float32)
        self.assertIs(pad_bluetooth_preroll(pcm, 24000, preroll_s=0.5, is_bluetooth_fn=lambda: False), pcm)
        self.assertIs(pad_bluetooth_preroll(pcm, 24000, preroll_s=0.5, is_bluetooth_fn=lambda: True, warm_duration_fn=lambda: 0.6), pcm)

        padded = pad_bluetooth_preroll(pcm, 24000, preroll_s=0.5, is_bluetooth_fn=lambda: True, warm_duration_fn=lambda: 0.2)
        self.assertEqual(len(padded), 9600)
        self.assertTrue(np.any(padded[:7200] != 0.0))
        self.assertLessEqual(float(np.max(np.abs(padded[:7200]))), 0.001)
        self.assertTrue(np.allclose(padded[7200:], 0.25))

    def test_word_timings_shift_forward_by_bluetooth_pilot_preroll(self):
        import numpy as np

        sr = 24000
        pcm = np.full(sr // 2, 0.3, dtype=np.float32)
        base_timings = compute_word_timings_ms(pcm, sr, "Hello world", lead_ms=0)
        padded = pad_bluetooth_preroll(pcm, sr, preroll_s=0.6, is_bluetooth_fn=lambda: True)
        padded_timings = compute_word_timings_ms(padded, sr, "Hello world", lead_ms=0)
        self.assertEqual(padded_timings[0] - base_timings[0], 600)


class BluetoothKeepaliveTests(unittest.TestCase):
    def test_keepalive_reuses_stream_generates_pilot_and_auto_sleeps(self):
        import time
        import numpy as np
        from voice.core.bluetooth_keepalive import BluetoothKeepalive

        streams = []

        class FakeStream:
            def __init__(self, **kw):
                self.kw = kw
                self.active = False
                self.events = []
                streams.append(self)

            def start(self):
                self.active = True
                self.events.append("start")

            def stop(self):
                self.active = False
                self.events.append("stop")

            def close(self):
                self.events.append("close")

        keepalive = BluetoothKeepalive(
            idle_timeout_s=0.12,
            is_bluetooth_fn=lambda: True,
            get_output_name_fn=lambda: "Galaxy Buds2 (5B05)",
            stream_factory=FakeStream,
        )
        try:
            self.assertTrue(keepalive.warm())
            self.assertTrue(keepalive.warm())
            self.assertEqual(len(streams), 1)
            buf = np.zeros((480, 1), dtype=np.float32)
            streams[0].kw["callback"](buf, 480, None, None)
            self.assertTrue(np.any(buf != 0.0))
            self.assertLessEqual(float(np.max(np.abs(buf))), 0.001)
            time.sleep(0.06)
            self.assertGreaterEqual(keepalive.warm_duration_s(), 0.04)
            time.sleep(0.18)
            self.assertEqual(keepalive.warm_duration_s(), 0.0)
            self.assertEqual(streams[0].events, ["start", "stop", "close"])
        finally:
            keepalive.stop()

    def test_sync_output_device_stops_keepalive_before_portaudio_refresh(self):
        from unittest.mock import patch

        devices = [{"index": 0, "name": "Mic"}, {"index": 1, "name": "MacBook Air Speakers"}]
        sd = _fake_sd(devices, [0, 1])
        order = []
        orig_terminate = sd._terminate
        sd._terminate = lambda: (order.append("terminate"), orig_terminate())[1]
        with patch("voice.core.bluetooth_keepalive.stop_bluetooth_keepalive", side_effect=lambda: order.append("stop_keepalive")):
            self.assertTrue(sync_output_device(sd, get_live_output_name=lambda: "Galaxy Buds2 (5B05)"))
        self.assertEqual(order, ["stop_keepalive", "terminate"])


if __name__ == "__main__":
    unittest.main()
