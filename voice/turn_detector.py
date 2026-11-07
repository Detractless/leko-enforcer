"""Zero-model Turn taking detector combining adaptive energy VAD, ambient noise calibration,
and p99lab turn-1-mini acoustic end-of-turn classifier.
"""
from __future__ import annotations

import collections
import os
import re
import time
from pathlib import Path
from typing import Optional, Tuple, Callable
import numpy as np

STEP = 2560
STATE = ("audio", "conv1", "conv2", "k", "v", "k_valid", "h6")
DEFAULT_MODEL = Path("/Users/calibro/.t3/scratch/phone-pilot/talk/models/turn-1-mini.step.int8dyn.onnx")

DANGLING_WORDS = {
    "and", "but", "so", "or", "because", "the", "a", "an", "to", "of", "um", "uh", "like", "if",
    "with", "for", "my", "our", "your", "then", "also", "about", "from", "as", "at",
    "need", "think", "want", "would", "could", "should", "number", "called"
}

def looks_unfinished(text: str) -> bool:
    """True if text trailing token is grammatically dangling (e.g. 'I don't think I need')."""
    toks = re.findall(r"[a-z']+", text.lower())
    return bool(toks) and toks[-1] in DANGLING_WORDS

class TurnDetector:
    def __init__(
        self,
        model_path: Optional[Path] = None,
        speech_threshold: float = 0.010,
        min_sil_frames: int = 4,   # 4 * 160ms = 640ms quiet before checking acoustic model
        max_sil_frames: int = 8,   # 8 * 160ms = 1.28s quiet ends turn unconditionally
        turn_threshold: float = 0.65,
        preroll_frames: int = 5,   # 5 * 160ms = 800ms rolling lead-in audio buffer
        on_speech_onset: Optional[Callable[[], None]] = None
    ):
        self.speech_threshold = speech_threshold
        self.min_sil_frames = min_sil_frames
        self.max_sil_frames = max_sil_frames
        self.turn_threshold = turn_threshold
        self.preroll_frames = preroll_frames
        self.preroll: collections.deque[np.ndarray] = collections.deque(maxlen=preroll_frames)
        self.on_speech_onset = on_speech_onset
        self.ambient_rms = collections.deque(maxlen=100)

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
        self.preroll.clear()
        if self.sess:
            self.state = {n: np.zeros(s, np.float32) for n, s in self.shapes.items()}

    def feed_ambient(self, chunk_i16: np.ndarray) -> None:
        """Dynamically calibrates speech threshold to ambient room noise floor."""
        a = np.asarray(chunk_i16)
        x = a.astype(np.float32) / 32768.0 if a.dtype == np.int16 else a.astype(np.float32)
        rms = float(np.sqrt(np.mean(x ** 2))) if len(x) > 0 else 0.0
        self.ambient_rms.append(rms)

        if len(self.ambient_rms) >= 5:
            p20 = float(np.percentile(self.ambient_rms, 20))
            new_thr = min(0.04, max(0.005, p20 * 3.5))
            self.speech_threshold = new_thr

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
            self.feed_ambient(chunk_i16)
            if rms > self.speech_threshold:
                self.in_speech = True
                self.sil_count = 0
                self.buf = list(self.preroll) + [x]
                self.preroll.clear()
                if self.on_speech_onset:
                    try:
                        self.on_speech_onset()
                    except Exception:
                        pass
            else:
                self.preroll.append(x)
            return False, np.zeros(0, dtype=np.float32)

        # In active speech turn
        self.buf.append(x)
        is_quiet = rms < (self.speech_threshold * 0.70)

        if is_quiet:
            self.sil_count += 1
            if self.sil_count >= self.min_sil_frames:
                if (p_end is not None and p_end > self.turn_threshold) or self.sil_count >= self.max_sil_frames:
                    full_audio = np.concatenate(self.buf)
                    self.reset()
                    return True, full_audio
        else:
            self.sil_count = 0

        return False, np.zeros(0, dtype=np.float32)


class TurnBufferMerger:
    """Buffers finished speech turns during grace period to merge ongoing speech clauses."""
    def __init__(self, grace_period_s: float = 0.8):
        self.grace_period_s = grace_period_s
        self.pending_turn: Optional[np.ndarray] = None
        self.turn_time: float = 0.0

    def push_turn(self, audio_f32: np.ndarray) -> None:
        self.pending_turn = audio_f32
        self.turn_time = time.time()

    def is_pending_continuation(self) -> bool:
        if self.pending_turn is None:
            return False
        return (time.time() - self.turn_time) <= self.grace_period_s

    def merge_continuation(self, new_audio_f32: np.ndarray) -> np.ndarray:
        if self.pending_turn is None:
            return new_audio_f32
        merged = np.concatenate([self.pending_turn, new_audio_f32])
        self.pending_turn = None
        return merged

    def clear(self) -> None:
        self.pending_turn = None
