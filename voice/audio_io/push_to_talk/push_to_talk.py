"""Manual hold-to-speak turn capture with a short release grace."""
from __future__ import annotations

import threading
import time

import numpy as np

from audio_io.capture_gate import CaptureGate


class PushToTalkDetector:
    preroll_frames = 0

    def __init__(self, grace_s: float = 2.0):
        self.grace_s = max(0.0, float(grace_s))
        self.on_expired = None
        self._events = threading.RLock()
        self._lock = threading.RLock()
        self._holding = False
        self._release_at = None
        self._timer = None
        self._chunks = []
        self._in_speech = False

    @property
    def capture_enabled(self) -> bool:
        with self._lock:
            return self._holding or self._release_at is not None

    @property
    def in_speech(self) -> bool:
        with self._lock:
            return self._in_speech

    @property
    def mode(self) -> str:
        with self._lock:
            if self._holding:
                return "held"
            return "grace" if self._release_at is not None else "idle"

    def _cancel_timer_locked(self):
        if self._timer is not None:
            self._timer.cancel()
            self._timer = None

    def _audio_locked(self):
        if not self._chunks:
            return np.zeros(0, dtype=np.float32)
        return np.concatenate(self._chunks).astype(np.float32, copy=False)

    def _clear_locked(self):
        self._cancel_timer_locked()
        self._holding = False
        self._release_at = None
        self._chunks.clear()
        self._in_speech = False

    def reset(self):
        with self._lock:
            self._clear_locked()

    def set_echo_cancellation_enabled(self, _enabled):
        return None

    def set_playback_active(self, _active, post_mute_s=0.25):
        return None

    def _schedule_expiry_locked(self, deadline):
        self._cancel_timer_locked()
        self._timer = threading.Timer(
            max(0.0, deadline - time.monotonic()), self._expire, args=(deadline,))
        self._timer.daemon = True
        self._timer.start()

    def _complete(self, audio):
        if self.on_expired:
            self.on_expired(audio)

    def _expire(self, deadline):
        with self._events:
            with self._lock:
                if self._holding or self._release_at != deadline:
                    return
                remaining = deadline - time.monotonic()
                if remaining > 0:
                    self._schedule_expiry_locked(deadline)
                    return
                audio = self._audio_locked()
                self._clear_locked()
            self._complete(audio)

    def begin_hold(self) -> bool:
        with self._events:
            completed = None
            with self._lock:
                if self._holding:
                    return False
                if self._release_at is not None:
                    if time.monotonic() >= self._release_at:
                        completed = self._audio_locked()
                        self._clear_locked()
                    else:
                        self._cancel_timer_locked()
                        self._release_at = None
                        self._holding = True
                        return True
                else:
                    self._chunks.clear()
                    self._in_speech = False
                    self._holding = True
                    return True
            self._complete(completed)
            with self._lock:
                self._chunks.clear()
                self._in_speech = False
                self._holding = True
            return True

    def release_hold(self) -> bool:
        with self._events:
            with self._lock:
                if not self._holding:
                    return False
                self._holding = False
                self._release_at = time.monotonic() + self.grace_s
                self._schedule_expiry_locked(self._release_at)
                return True

    def cancel(self):
        with self._events:
            with self._lock:
                self._clear_locked()

    def feed(self, chunk_i16):
        empty = np.zeros(0, dtype=np.float32)
        with self._lock:
            if not (self._holding or self._release_at is not None):
                return False, empty
            if self._release_at is not None and time.monotonic() >= self._release_at:
                return False, empty
            samples = np.asarray(chunk_i16, dtype=np.int16).reshape(-1)
            if samples.size:
                self._chunks.append(samples.astype(np.float32) / 32768.0)
                self._in_speech = True
            return False, empty


class PushToTalkTurnManager(CaptureGate):
    def __init__(self, stream_factory, echo_audio, on_turn_complete,
                 sample_rate=16000, chunk_size=1280, on_speech_onset=None,
                 on_armed=None, on_capture_error=None, grace_s=2.0):
        detector = PushToTalkDetector(grace_s=grace_s)
        super().__init__(stream_factory, detector, echo_audio, on_turn_complete,
                         sample_rate=sample_rate, chunk_size=chunk_size,
                         on_armed=on_armed or on_speech_onset,
                         on_capture_error=on_capture_error)
        detector.on_expired = self._on_expired

    def _on_expired(self, audio):
        try:
            self._finish_turn(audio)
        finally:
            with self._cv:
                self._cv.notify_all()

    @property
    def capture_mode(self):
        return self.detector.mode

    def begin_capture(self):
        with self._cv:
            self._muted = False
        started = self.detector.begin_hold()
        if started:
            with self._cv:
                self._cv.notify_all()
        return started

    def end_capture(self):
        released = self.detector.release_hold()
        if released:
            with self._cv:
                self._cv.notify_all()
        return released

    def cancel_capture(self):
        self.detector.cancel()
        with self._cv:
            self._clear()
            self._cv.notify_all()
