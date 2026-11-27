"""Tests for Bluetooth headset mic avoidance during stream recovery."""
import sys
import unittest
from pathlib import Path
from types import SimpleNamespace

sys.path.insert(0, str(Path(__file__).resolve().parents[4]))
from voice.core.stream_recovery import open_input_stream, resolve_input_device


def _fake_sd(devices, default_pair):
    opened = []

    def raw_stream(**kw):
        opened.append(kw)
        return "stream"

    return SimpleNamespace(
        default=SimpleNamespace(device=default_pair),
        query_devices=lambda: devices,
        RawInputStream=raw_stream,
        opened=opened,
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


if __name__ == "__main__":
    unittest.main()
