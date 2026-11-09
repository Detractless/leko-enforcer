"""RNNoise speech probabilities for 16 kHz mono microphone audio."""
import ctypes
import importlib.metadata
import sys

import numpy as np
import soxr

try:
    from .mono_audio import finite_mono
except ImportError:
    from mono_audio import finite_mono


class RNNoiseVAD:
    """Use the native RNNoise frame probability; retain existing echo processing."""

    def __init__(self):
        distribution = importlib.metadata.distribution("pyrnnoise")
        if distribution.version != "0.4.3":
            raise RuntimeError("comparison requires pyrnnoise==0.4.3")
        libraries = [path for path in distribution.files or () if str(path).endswith("/librnnoise.dylib")]
        if len(libraries) != 1:
            raise RuntimeError("pyrnnoise native library not found")
        self._lib = ctypes.CDLL(str(distribution.locate_file(libraries[0])))
        self._lib.rnnoise_create.argtypes = [ctypes.c_void_p]
        self._lib.rnnoise_create.restype = ctypes.c_void_p
        self._lib.rnnoise_destroy.argtypes = [ctypes.c_void_p]
        self._lib.rnnoise_process_frame.argtypes = [ctypes.c_void_p, ctypes.POINTER(ctypes.c_float), ctypes.POINTER(ctypes.c_float)]
        self._lib.rnnoise_process_frame.restype = ctypes.c_float
        self._lib.rnnoise_get_frame_size.restype = ctypes.c_int
        if self._lib.rnnoise_get_frame_size() != 480:
            raise RuntimeError("unexpected RNNoise frame size")
        self._state = None
        self._failed = False
        self.reset()
        if self._failed:
            raise RuntimeError("RNNoise failed to initialize")

    def reset(self):
        if self._failed:
            return
        try:
            if self._state:
                self._lib.rnnoise_destroy(self._state)
                self._state = None
            self._state = self._lib.rnnoise_create(None)
            if not self._state:
                raise RuntimeError("RNNoise could not create state")
            self._resampler = soxr.ResampleStream(16000, 48000, 1, dtype="float32", quality="HQ")
            self._pending = np.empty(0, dtype=np.float32)
        except Exception as exc:
            self._disable(exc)

    def _disable(self, exc):
        self._failed = True
        print(f"[RNNoise speech detector failed; listening disabled]: {exc}", file=sys.stderr, flush=True)

    def process(self, audio):
        if self._failed:
            return 0.0
        try:
            samples = finite_mono(audio, "RNNoise")
            converted = self._resampler.resample_chunk(np.clip(samples, -1.0, 1.0))
            self._pending = np.concatenate((self._pending, converted * 32767.0))
            probabilities = []
            while len(self._pending) >= 480:
                frame = np.ascontiguousarray(self._pending[:480], dtype=np.float32)
                self._pending = self._pending[480:]
                output = np.empty_like(frame)
                prob = float(self._lib.rnnoise_process_frame(
                    self._state, output.ctypes.data_as(ctypes.POINTER(ctypes.c_float)),
                    frame.ctypes.data_as(ctypes.POINTER(ctypes.c_float))))
                if not np.isfinite(prob) or not 0.0 <= prob <= 1.0:
                    raise ValueError("invalid RNNoise speech probability")
                probabilities.append(prob)
            return max(probabilities) if probabilities else None
        except Exception as exc:
            self._disable(exc)
            return 0.0

    def __del__(self):
        if getattr(self, "_state", None):
            self._lib.rnnoise_destroy(self._state)
            self._state = None
