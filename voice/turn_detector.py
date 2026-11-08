"""Speech onset detection and p99lab turn-1-mini end-of-turn classification."""
from __future__ import annotations

import collections
import os
import sys
import time
from pathlib import Path
from typing import Optional, Tuple, Callable
import numpy as np

STEP = 2560
STATE = ("audio", "conv1", "conv2", "k", "v", "k_valid", "h6")
DEFAULT_MODEL = Path("/Users/calibro/.t3/scratch/phone-pilot/talk/models/turn-1-mini.step.int8dyn.onnx")

class TurnDetector:
    def __init__(
        self,
        model_path: Optional[Path] = None,
        speech_threshold: float = 0.010,
        barge_in_threshold: float = 0.035,
        min_sil_frames: int = 4,   # 4 * 160ms = 640ms quiet before checking acoustic model
        max_sil_frames: int = 8,   # 8 * 160ms = 1.28s quiet ends turn unconditionally
        turn_threshold: float = 0.65,
        preroll_frames: int = 5,   # 5 * 160ms = 800ms rolling lead-in audio buffer
        max_turn_duration_s: float = 60.0,  # Generous cap for long thoughtful turns
        on_speech_onset: Optional[Callable[[], None]] = None,
        speech_detector=None,
        continuation_s: float = 0.96,
    ):
        self.continuation_s = continuation_s
        self.quiet_samples = 0
        self.endpoint_pending = False
        self.speech_threshold = speech_threshold
        self.barge_in_threshold = barge_in_threshold
        self.min_sil_frames = min_sil_frames
        self.max_sil_frames = max_sil_frames
        self.turn_threshold = turn_threshold
        self.preroll_frames = preroll_frames
        self.max_turn_duration_s = max_turn_duration_s
        self.preroll: collections.deque[np.ndarray] = collections.deque(maxlen=preroll_frames)
        self.on_speech_onset = on_speech_onset
        self.speech_detector = None
        if speech_detector is not False:
            try:
                if speech_detector is None:
                    try:
                        from .audio_io.silero_vad import SileroVAD
                    except ImportError:
                        from audio_io.silero_vad import SileroVAD
                    speech_detector = SileroVAD()
                self.speech_detector = speech_detector
            except Exception as exc:
                print(f"[Speech detector unavailable; using loudness fallback]: {exc}", file=sys.stderr, flush=True)
        self.ambient_rms = collections.deque(maxlen=100)
        self.playback_active = False
        self.echo_cancellation_enabled = False
        self.echo_guard_until = 0.0
        self._echo_guard_reset = False
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
        self.quiet_samples = 0
        self.endpoint_pending = False
        self.in_speech = False
        self.sil_count = 0
        self.buf = []
        self.pending = np.zeros(0, np.float32)
        self.preroll.clear()
        if self.speech_detector is not None:
            try:
                self.speech_detector.reset()
            except Exception as exc:
                print(f"[Speech detector failed; using loudness fallback]: {exc}", file=sys.stderr, flush=True)
                self.speech_detector = None
        if self.sess:
            self.state = {n: np.zeros(s, np.float32) for n, s in self.shapes.items()}

    def set_playback_active(self, active: bool, post_mute_s: float = 0.25) -> None:
        """Discard microphone input during playback and brief room echo decay."""
        was_active = self.playback_active
        self.playback_active = active
        if active:
            self.echo_guard_until = 0.0
            self._reset_for_echo_guard()
        elif was_active:
            self.echo_guard_until = time.monotonic() + post_mute_s

    def set_echo_cancellation_enabled(self, enabled: bool) -> None:
        """Allows full-duplex listening only when render-reference AEC is active."""
        self.echo_cancellation_enabled = enabled
        if self.playback_active and not enabled:
            self._reset_for_echo_guard()

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
        is_decaying = not self.playback_active and time.monotonic() < self.echo_guard_until
        if is_decaying or self.playback_active:
            self._reset_for_echo_guard()
            return False, np.zeros(0, dtype=np.float32)

        if self._echo_guard_reset:
            self.reset()
            self._echo_guard_reset = False

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

        if self.playback_active:
            if self.speech_detector is not None:
                # Neural speech detectors classify speaker playback as speech.
                # Barge-in requires acoustic energy above the speaker echo floor.
                effective_threshold = self.barge_in_threshold
            elif not self.echo_cancellation_enabled:
                effective_threshold = self.barge_in_threshold
            else:
                effective_threshold = self.speech_threshold
        else:
            effective_threshold = self.speech_threshold

        detector_prob = None
        if self.speech_detector is not None:
            try:
                detector_prob = self.speech_detector.process(x)
            except Exception as exc:
                print(f"[Speech detector failed; using loudness fallback]: {exc}", file=sys.stderr, flush=True)
                self.speech_detector = None

        if not self.in_speech:
            if not self.playback_active:
                self.feed_ambient(chunk_i16)

            if self.playback_active:
                # During playback, Maeve's own speaker echo will trigger neural VADs.
                # Barge-in requires genuine vocal energy above the echo floor.
                speech_started = rms > effective_threshold
                if self.speech_detector is not None and detector_prob is not None:
                    speech_started = speech_started and (detector_prob >= 0.5)
            else:
                if self.speech_detector is not None and detector_prob is not None:
                    speech_started = detector_prob >= 0.5
                else:
                    speech_started = rms > effective_threshold
            if speech_started:
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

        # Hard cap: prevent indefinite listening if silence is never met
        turn_duration_s = len(self.buf) * len(x) / 16000.0
        if turn_duration_s >= self.max_turn_duration_s:
            full_audio = np.concatenate(self.buf)
            self.reset()
            return True, full_audio

        # Determine if current frame is quiet
        if self.speech_detector is not None and detector_prob is not None:
            is_quiet = detector_prob < 0.35
        else:
            is_quiet = rms < self.speech_threshold

        if is_quiet:
            self.sil_count += 1
            self.quiet_samples += len(x)
            if self.sil_count >= self.min_sil_frames:
                if (p_end is not None and p_end > self.turn_threshold) or self.sil_count >= self.max_sil_frames:
                    self.endpoint_pending = True
            # Count existing quiet toward the continuation window, rather than
            # adding another 900ms after the acoustic/fallback endpoint.
            if self.endpoint_pending and self.quiet_samples >= self.continuation_s * 16000:
                full_audio = np.concatenate(self.buf)
                self.reset()
                return True, full_audio
        else:
            # Continued speech keeps the same buffer and invalidates any endpoint.
            self.sil_count = 0
            self.quiet_samples = 0
            self.endpoint_pending = False

        return False, np.zeros(0, dtype=np.float32)

    def _reset_for_echo_guard(self) -> None:
        if not self._echo_guard_reset:
            self.reset()
            self._echo_guard_reset = True
