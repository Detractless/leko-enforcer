"""Streaming 16 kHz speech probabilities from the bundled Silero ONNX model."""
from pathlib import Path
from typing import Optional

import numpy as np

MODEL_PATH = Path(__file__).resolve().parent / "models" / "silero-vad-v5.1.2.onnx"
FRAME_SAMPLES = 512
CONTEXT_SAMPLES = 64


class SileroVAD:
    """Keep Silero's recurrent state and context across microphone chunks.

    Initialization and inference errors propagate to the caller so it can
    switch to its fallback detector. Input is mono float audio at 16 kHz.
    """

    def __init__(self, model_path: Path = MODEL_PATH):
        import onnxruntime as ort

        options = ort.SessionOptions()
        options.intra_op_num_threads = 1
        options.inter_op_num_threads = 1
        self.session = ort.InferenceSession(
            str(model_path), sess_options=options, providers=["CPUExecutionProvider"]
        )
        self.reset()

    def reset(self) -> None:
        self._state = np.zeros((2, 1, 128), dtype=np.float32)
        self._context = np.zeros((1, CONTEXT_SAMPLES), dtype=np.float32)
        self._pending = np.empty(0, dtype=np.float32)

    def process(self, audio: np.ndarray) -> Optional[float]:
        """Return the highest frame probability, or None for an incomplete frame."""
        audio = np.asarray(audio, dtype=np.float32)
        if audio.ndim != 1 or not np.isfinite(audio).all():
            raise ValueError("Silero requires finite, mono 16 kHz audio")
        self._pending = np.concatenate((self._pending, audio))
        probabilities = []
        while len(self._pending) >= FRAME_SAMPLES:
            frame = self._pending[:FRAME_SAMPLES].reshape(1, FRAME_SAMPLES)
            self._pending = self._pending[FRAME_SAMPLES:]
            model_input = np.concatenate((self._context, frame), axis=1)
            output, self._state = self.session.run(
                None,
                {"input": model_input, "state": self._state,
                 "sr": np.array(16000, dtype=np.int64)},
            )
            probability = float(np.asarray(output).reshape(-1)[0])
            if not np.isfinite(probability) or not 0.0 <= probability <= 1.0:
                raise ValueError("Silero returned an invalid speech probability")
            self._context = model_input[:, -CONTEXT_SAMPLES:].copy()
            probabilities.append(probability)
        return max(probabilities) if probabilities else None
