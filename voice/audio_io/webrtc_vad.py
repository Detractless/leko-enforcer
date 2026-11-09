"""WebRTC speech-start detection for 16 kHz microphone audio."""
import sys

import numpy as np

try:
    from .mono_audio import finite_mono
except ImportError:
    from mono_audio import finite_mono

FRAME_SAMPLES = 320  # WebRTC accepts 10, 20 or 30 ms frames; this is 20 ms.
SAMPLE_RATE = 16000


class WebRTCVAD:
    """Adapt WebRTC's binary PCM detector to the turn detector interface."""

    def __init__(self, mode=2):
        import webrtcvad

        self._module = webrtcvad
        self._mode = mode
        self._failed = False
        self.reset()

    def reset(self):
        if self._failed:
            return
        try:
            self._vad = self._module.Vad(self._mode)
            self._pending = np.empty(0, dtype=np.float32)
        except Exception as exc:
            self._disable(exc)

    def _disable(self, exc):
        self._failed = True
        print(f"[WebRTC speech detector failed; listening disabled]: {exc}", file=sys.stderr, flush=True)

    def process(self, audio):
        if self._failed:
            return 0.0
        try:
            samples = finite_mono(audio, "WebRTC VAD")
            self._pending = np.concatenate((self._pending, samples))
            speech = False
            found = False
            while len(self._pending) >= FRAME_SAMPLES:
                frame = self._pending[:FRAME_SAMPLES]
                self._pending = self._pending[FRAME_SAMPLES:]
                pcm = (np.clip(frame, -1.0, 1.0) * 32767).astype('<i2').tobytes()
                speech = self._vad.is_speech(pcm, SAMPLE_RATE) or speech
                found = True
            return float(speech) if found else None
        except Exception as exc:
            self._disable(exc)
            return 0.0
