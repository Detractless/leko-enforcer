"""Speaker isolation and bounded loudness normalization via alkove."""
from __future__ import annotations

import json
import threading
import time
from pathlib import Path
from typing import Optional, Tuple

import numpy as np

MIN_SPEECH_RMS = 0.0015
TARGET_RMS = 0.05
MAX_GAIN = 12.0
CHUNK_SAMPLES = 1600
LEKO_SUPPORT_DIR = Path.home() / "Library/Application Support/Leko"
VOICEPRINT_PATH = LEKO_SUPPORT_DIR / "voiceprint.json"
SETTINGS_PATH = LEKO_SUPPORT_DIR / "leko-settings.json"


def max_chunk_rms(audio_f32: np.ndarray, chunk_size: int = CHUNK_SAMPLES) -> float:
    """Return peak 100ms RMS energy across complete chunks (or whole clip if shorter)."""
    audio = np.asarray(audio_f32, dtype=np.float32).reshape(-1)
    if not audio.size:
        return 0.0
    num_chunks = len(audio) // chunk_size
    if num_chunks <= 0:
        return float(np.sqrt(np.mean(audio * audio)))
    return max(
        float(np.sqrt(np.mean(audio[i * chunk_size:(i + 1) * chunk_size] ** 2)))
        for i in range(num_chunks)
    )


def normalize_quiet_speech(
    audio_f32: np.ndarray,
    min_rms: float = MIN_SPEECH_RMS,
    target_rms: float = TARGET_RMS,
    max_gain: float = MAX_GAIN,
) -> Optional[np.ndarray]:
    """Boost quiet speech safely without amplifying sub-floor silence."""
    audio = np.asarray(audio_f32, dtype=np.float32).reshape(-1)
    if len(audio) < 3200:
        return None
    rms = max_chunk_rms(audio)
    if rms < min_rms:
        return None
    if rms < target_rms:
        gain = min(max_gain, target_rms / rms)
        peak = float(np.max(np.abs(audio)))
        if peak * gain > 0.95:
            gain = 0.95 / max(peak, 1e-6)
        audio = audio * gain
    return np.ascontiguousarray(np.clip(audio, -1.0, 1.0), dtype=np.float32)


def _isolation_enabled(settings_path: Path = SETTINGS_PATH) -> bool:
    try:
        data = json.loads(settings_path.read_text(encoding="utf-8"))
        if isinstance(data, dict) and "speaker_isolation" in data:
            return bool(data["speaker_isolation"])
    except (OSError, json.JSONDecodeError, ValueError):
        pass
    return True


class AlkoveIsolator:
    """Lazy-loads Focus/Timbre models and isolates the user's voice on 16kHz audio."""

    def __init__(self, voiceprint_path: Path = VOICEPRINT_PATH, settings_path: Path = SETTINGS_PATH):
        self.voiceprint_path = Path(voiceprint_path)
        self.settings_path = Path(settings_path)
        self._lock = threading.Lock()
        self._alkove = None
        self._models = None
        self._voiceprint = None
        self._vp_mtime: Optional[float] = None
        self._failed = False

    def _ensure_loaded(self) -> bool:
        if self._failed:
            return False
        with self._lock:
            if self._models is not None:
                return True
            try:
                import alkove

                self._alkove = alkove
                self._models = alkove.Models.load(["focus", "cue", "timbre"])
                return True
            except Exception:
                self._failed = True
                return False

    def _load_voiceprint(self):
        if not self._alkove or not self.voiceprint_path.exists():
            self._voiceprint = None
            self._vp_mtime = None
            return None
        try:
            mtime = self.voiceprint_path.stat().st_mtime
            if self._voiceprint is not None and self._vp_mtime == mtime:
                return self._voiceprint
            raw = self.voiceprint_path.read_text(encoding="utf-8")
            data = json.loads(raw) if raw.lstrip().startswith("{") else raw
            if isinstance(data, dict) and isinstance(data.get("voiceprint"), dict):
                data = data["voiceprint"]
            self._voiceprint = self._alkove.Voiceprint.from_json(data)
            self._vp_mtime = mtime
            return self._voiceprint
        except Exception:
            self._voiceprint = None
            self._vp_mtime = None
            return None

    def isolate(self, audio_f32: np.ndarray, sample_rate: int = 16000) -> Tuple[np.ndarray, bool]:
        """Return `(processed_audio, used_alkove)` with graceful passthrough on errors."""
        audio = np.asarray(audio_f32, dtype=np.float32).reshape(-1)
        if not _isolation_enabled(self.settings_path) or not self._ensure_loaded():
            return audio, False
        try:
            vp = self._load_voiceprint()
            out = self._alkove.process(
                self._models,
                audio,
                rate=sample_rate,
                focus=(vp is None),
                voiceprint=vp,
                cue=False,
                normalize=True,
            ).audio
            cleaned = np.asarray(out, dtype=np.float32).reshape(-1)
            if not cleaned.size:
                return audio, False
            return cleaned, True
        except Exception:
            return audio, False


class IdleSafeEchoAudio:
    """Wrap EchoAudioProcessor so idle turns bypass AEC without over-suppressing speech."""

    def __init__(self, inner, render_window_s: float = 0.6):
        self._inner = inner
        self._render_window_s = float(render_window_s)
        self._lock = threading.Lock()
        self._last_render_at = 0.0

    def __getattr__(self, name):
        return getattr(self._inner, name)

    @property
    def available(self) -> bool:
        return bool(self._inner.available)

    def reset_buffers(self) -> None:
        with self._lock:
            self._last_render_at = 0.0
        self._inner.reset_buffers()

    def feed_render(self, samples: np.ndarray) -> bool:
        with self._lock:
            self._last_render_at = time.monotonic()
        return self._inner.feed_render(samples)

    def clean_capture(self, samples_i16: np.ndarray):
        with self._lock:
            active = self._last_render_at > 0.0 and (time.monotonic() - self._last_render_at) <= self._render_window_s
        if not active:
            return np.asarray(samples_i16, dtype=np.int16)
        return self._inner.clean_capture(samples_i16)


_DEFAULT_ISOLATOR = AlkoveIsolator()


def prepare_audio_for_stt(
    audio_f32: np.ndarray,
    isolator: Optional[AlkoveIsolator] = None,
) -> Optional[np.ndarray]:
    """Validate speech energy, run Alkove isolation when available, and normalize volume."""
    normalized = normalize_quiet_speech(audio_f32)
    if normalized is None:
        return None
    target = isolator or _DEFAULT_ISOLATOR
    isolated, used = target.isolate(np.asarray(audio_f32, dtype=np.float32).reshape(-1))
    if not used:
        return normalized
    if max_chunk_rms(isolated) < MIN_SPEECH_RMS * 0.5:
        return None
    return np.ascontiguousarray(np.clip(isolated, -1.0, 1.0), dtype=np.float32)
