"""Turn taking detector combining adaptive energy VAD and turn-1-mini acoustic end-of-turn classifier.
Adapted from phone-pilot.
"""
from __future__ import annotations

import os
from pathlib import Path
from typing import Optional, Tuple
import numpy as np

STEP = 2560
STATE = ("audio", "conv1", "conv2", "k", "v", "k_valid", "h6")
DEFAULT_MODEL = Path("/Users/calibro/.t3/scratch/phone-pilot/talk/models/turn-1-mini.step.int8dyn.onnx")

class TurnDetector:
    def __init__(self, model_path: Optional[Path] = None, speech_threshold: float = 0.012, sil_frames: int = 4):
        self.speech_threshold = speech_threshold
        self.sil_frames = sil_frames
        self.sess = None
        path = model_path or DEFAULT_MODEL
        if path.exists():
            try:
                import onnxruntime as ort
                so = ort.SessionOptions()
                so.intra_op_num_threads = so.inter_op_num_threads = 1
                self.sess = ort.InferenceSession(str(path), so, providers=["CPUExecutionProvider"])
                self.shapes = {i.name: tuple(i.shape) for i in self.sess.get_inputs() if i.name.startswith("state_")}
                self.out = ["probs"] + [f"new_state_{s}" for s in STATE]
                self.state = {n: np.zeros(s, np.float32) for n, s in self.shapes.items()}
            except Exception:
                self.sess = None

        self.in_speech = False
        self.sil_count = 0
        self.buf: list[np.ndarray] = []
        self.pending = np.zeros(0, np.float32)

    def reset(self) -> None:
        self.in_speech = False
        self.sil_count = 0
        self.buf = []
        self.pending = np.zeros(0, np.float32)
        if self.sess:
            self.state = {n: np.zeros(s, np.float32) for n, s in self.shapes.items()}

    def feed(self, chunk_i16: np.ndarray) -> Tuple[bool, np.ndarray]:
        """Feeds an audio chunk (16 kHz int16).
        Returns (turn_ended, full_turn_audio_f32).
        """
        a = np.asarray(chunk_i16)
        x = a.astype(np.float32) / 32768.0 if a.dtype == np.int16 else a.astype(np.float32)
        rms = float(np.sqrt(np.mean(x ** 2))) if len(x) > 0 else 0.0

        p_end = None
        if self.sess:
            self.pending = np.concatenate([self.pending, x])
            while len(self.pending) >= STEP:
                res = self.sess.run(self.out, {"audio": self.pending[:STEP].reshape(1, STEP), **self.state})
                self.state = {f"state_{s}": r for s, r in zip(STATE, res[1:])}
                p_end = float(res[0][-1])
                self.pending = self.pending[STEP:]

        if not self.in_speech:
            if rms > self.speech_threshold:
                self.in_speech = True
                self.sil_count = 0
                self.buf = [x]
            return False, np.zeros(0, dtype=np.float32)

        # In active speech turn
        self.buf.append(x)
        is_quiet = rms < (self.speech_threshold * 0.75)

        if is_quiet:
            self.sil_count += 1
            # Turn ends if model is confident of turn-end during pause or sustained silence
            if (p_end is not None and p_end > 0.45 and self.sil_count >= 2) or self.sil_count >= self.sil_frames:
                full_audio = np.concatenate(self.buf)
                self.reset()
                return True, full_audio
        else:
            self.sil_count = 0

        return False, np.zeros(0, dtype=np.float32)
