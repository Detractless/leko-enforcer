"""TEN VAD speech-start detector for 16 kHz microphone audio."""
import ctypes
import sys
import threading
from pathlib import Path

import numpy as np

LIBRARY = Path(__file__).resolve().parent / "models" / "ten-vad-8843b975.dylib"
FRAME_SAMPLES = 256


class TENVAD:
    """Use TEN's stateful native 16 ms detector on already-cleaned microphone audio."""

    def __init__(self, library_path=LIBRARY):
        self._lock = threading.RLock()
        self._lib = ctypes.CDLL(str(library_path))
        self._lib.ten_vad_create.argtypes = [ctypes.POINTER(ctypes.c_void_p), ctypes.c_size_t, ctypes.c_float]
        self._lib.ten_vad_create.restype = ctypes.c_int
        self._lib.ten_vad_process.argtypes = [ctypes.c_void_p, ctypes.POINTER(ctypes.c_int16), ctypes.c_size_t,
                                               ctypes.POINTER(ctypes.c_float), ctypes.POINTER(ctypes.c_int)]
        self._lib.ten_vad_process.restype = ctypes.c_int
        self._lib.ten_vad_destroy.argtypes = [ctypes.POINTER(ctypes.c_void_p)]
        self._lib.ten_vad_destroy.restype = ctypes.c_int
        self._handle = ctypes.c_void_p()
        self._failed = False
        self.reset()
        if self._failed:
            raise RuntimeError("TEN VAD failed to initialize")

    def _disable(self, exc):
        self._failed = True
        print(f"[TEN speech detector failed; listening disabled]: {exc}", file=sys.stderr, flush=True)

    def reset(self):
        with self._lock:
            if self._failed:
                return
            try:
                if self._handle.value and self._lib.ten_vad_destroy(ctypes.byref(self._handle)) != 0:
                    raise RuntimeError("could not destroy TEN VAD state")
                self._handle = ctypes.c_void_p()
                if self._lib.ten_vad_create(ctypes.byref(self._handle), FRAME_SAMPLES, 0.5) != 0 or not self._handle.value:
                    raise RuntimeError("could not create TEN VAD state")
                self._pending = np.empty(0, dtype=np.float32)
            except Exception as exc:
                self._disable(exc)

    def process(self, audio):
        with self._lock:
            if self._failed:
                return 0.0
            try:
                samples = np.asarray(audio, dtype=np.float32)
                if samples.ndim != 1 or not np.isfinite(samples).all():
                    raise ValueError("TEN VAD requires finite mono 16 kHz audio")
                self._pending = np.concatenate((self._pending, samples))
                probabilities = []
                while len(self._pending) >= FRAME_SAMPLES:
                    samples = self._pending[:FRAME_SAMPLES]
                    self._pending = self._pending[FRAME_SAMPLES:]
                    frame = np.ascontiguousarray(np.clip(samples, -1, 1) * 32767, dtype=np.int16)
                    probability, flag = ctypes.c_float(), ctypes.c_int()
                    code = self._lib.ten_vad_process(self._handle, frame.ctypes.data_as(ctypes.POINTER(ctypes.c_int16)),
                                                     FRAME_SAMPLES, ctypes.byref(probability), ctypes.byref(flag))
                    value = float(probability.value)
                    if code != 0 or not np.isfinite(value) or not 0.0 <= value <= 1.0:
                        raise RuntimeError("invalid TEN VAD inference result")
                    probabilities.append(value)
                return max(probabilities) if probabilities else None
            except Exception as exc:
                self._disable(exc)
                return 0.0

    def __del__(self):
        if getattr(self, "_handle", None) and self._handle.value:
            self._lib.ten_vad_destroy(ctypes.byref(self._handle))
