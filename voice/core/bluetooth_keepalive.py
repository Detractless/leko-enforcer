"""Keep macOS Bluetooth A2DP streams awake during turns with an inaudible pilot signal."""
from __future__ import annotations

import math
import threading
import time
from typing import Callable, Optional

import numpy as np

from .stream_recovery import _coreaudio_default_output_name, is_bluetooth_output

PILOT_AMPLITUDE = 0.00025  # ~-72 dBFS: non-zero in 16-bit AAC/SBC, below room floor
PILOT_FREQ_HZ = 180.0
WARM_TARGET_S = 0.65
DEFAULT_IDLE_TIMEOUT_S = 20.0


def make_bluetooth_pilot(
    sample_rate: int,
    duration_s: float,
    channels: int = 1,
    amplitude: float = PILOT_AMPLITUDE,
    freq_hz: float = PILOT_FREQ_HZ,
) -> np.ndarray:
    """Generate a low-level fade-in sine pilot so Bluetooth DACs unmute without audible sound."""
    n_samples = int(round(sample_rate * duration_s))
    if n_samples <= 0 or sample_rate <= 0:
        shape = (0, channels) if channels > 1 else (0,)
        return np.zeros(shape, dtype=np.float32)
    t = np.arange(n_samples, dtype=np.float32) / float(sample_rate)
    wave = (amplitude * np.sin((2.0 * math.pi * freq_hz) * t)).astype(np.float32)
    fade_len = min(n_samples, max(1, int(round(sample_rate * 0.01))))
    if fade_len > 1:
        ramp = 0.5 * (1.0 - np.cos(np.linspace(0.0, math.pi, fade_len, dtype=np.float32)))
        wave[:fade_len] *= ramp
    if channels > 1:
        return np.repeat(wave[:, None], channels, axis=1)
    return wave


class BluetoothKeepalive:
    """Own a lightweight non-blocking OutputStream to hold Bluetooth A2DP open across turns."""

    def __init__(
        self,
        *,
        idle_timeout_s: float = DEFAULT_IDLE_TIMEOUT_S,
        sample_rate: int = 24000,
        amplitude: float = PILOT_AMPLITUDE,
        freq_hz: float = PILOT_FREQ_HZ,
        is_bluetooth_fn: Optional[Callable[[], bool]] = None,
        get_output_name_fn: Optional[Callable[[], Optional[str]]] = None,
        clock_fn: Callable[[], float] = time.monotonic,
        stream_factory: Optional[Callable[..., object]] = None,
    ):
        self.idle_timeout_s = idle_timeout_s
        self.sample_rate = sample_rate
        self.amplitude = amplitude
        self.freq_hz = freq_hz
        self._is_bluetooth = is_bluetooth_fn or is_bluetooth_output
        self._get_output_name = get_output_name_fn or _coreaudio_default_output_name
        self._clock = clock_fn
        self._stream_factory = stream_factory
        self._lock = threading.RLock()
        self._stream = None
        self._started_at = 0.0
        self._expires_at = 0.0
        self._output_name: Optional[str] = None
        self._timer: Optional[threading.Timer] = None

    def _make_callback(self) -> Callable:
        phase = 0
        omega = 2.0 * math.pi * self.freq_hz / float(self.sample_rate)
        amp = self.amplitude

        def callback(outdata, frames, _time_info, _status):
            nonlocal phase
            idx = phase + np.arange(frames, dtype=np.float32)
            outdata[:, 0] = (amp * np.sin(omega * idx)).astype(np.float32)
            if outdata.shape[1] > 1:
                outdata[:, 1:] = outdata[:, :1]
            phase = (phase + frames) % self.sample_rate

        return callback

    def _schedule_timer_locked(self, delay_s: float) -> None:
        if self._timer is not None:
            self._timer.cancel()
        timer = threading.Timer(max(0.05, delay_s), self._on_idle_timer)
        timer.daemon = True
        self._timer = timer
        timer.start()

    def _on_idle_timer(self) -> None:
        with self._lock:
            if self._stream is None:
                return
            remaining = self._expires_at - self._clock()
            if remaining > 0.02:
                self._schedule_timer_locked(remaining)
                return
            self.stop()

    def warm(self, sd_module=None, hold_s: Optional[float] = None) -> bool:
        """Start or extend Bluetooth keepalive if Bluetooth output is currently active."""
        try:
            if not self._is_bluetooth():
                self.stop()
                return False
            live_name = self._get_output_name()
        except Exception:
            return False

        ttl = self.idle_timeout_s if hold_s is None else max(0.1, float(hold_s))
        now = self._clock()
        with self._lock:
            if self._stream is not None:
                active = getattr(self._stream, "active", True)
                if active and live_name == self._output_name:
                    self._expires_at = max(self._expires_at, now + ttl)
                    self._schedule_timer_locked(self._expires_at - now)
                    return True
                self.stop()

            factory = self._stream_factory
            if factory is None:
                if sd_module is None:
                    import sounddevice as sd_module
                factory = getattr(sd_module, "OutputStream", None)
            if not callable(factory):
                return False
            try:
                stream = factory(
                    samplerate=self.sample_rate,
                    channels=1,
                    dtype="float32",
                    callback=self._make_callback(),
                )
                stream.start()
            except Exception:
                return False
            self._stream = stream
            self._started_at = self._clock()
            self._expires_at = self._started_at + ttl
            self._output_name = live_name
            self._schedule_timer_locked(ttl)
            return True

    def warm_duration_s(self) -> float:
        """Return seconds the keepalive stream has been continuously active on Bluetooth."""
        with self._lock:
            if self._stream is None:
                return 0.0
            if not getattr(self._stream, "active", True):
                self.stop()
                return 0.0
            return max(0.0, self._clock() - self._started_at)

    def stop(self) -> None:
        """Close the keepalive stream and cancel any pending idle timer."""
        with self._lock:
            if self._timer is not None:
                self._timer.cancel()
                self._timer = None
            stream = self._stream
            self._stream = None
            self._started_at = 0.0
            self._expires_at = 0.0
            self._output_name = None
        if stream is not None:
            for fn_name in ("stop", "close"):
                fn = getattr(stream, fn_name, None)
                if callable(fn):
                    try:
                        fn()
                    except Exception:
                        pass


_default_keepalive = BluetoothKeepalive()


def warm_bluetooth_output(sd_module=None, hold_s: Optional[float] = None) -> bool:
    return _default_keepalive.warm(sd_module=sd_module, hold_s=hold_s)


def bluetooth_warm_duration_s() -> float:
    return _default_keepalive.warm_duration_s()


def stop_bluetooth_keepalive() -> None:
    _default_keepalive.stop()
