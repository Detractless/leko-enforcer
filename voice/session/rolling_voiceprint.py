"""Rolling 20-turn voiceprint learning using Alkove Timbre embeddings."""
from __future__ import annotations

import json
import os
import queue
import threading
import time
from collections import deque
from pathlib import Path
from typing import Callable, List, Optional, Tuple

import numpy as np

from ..core.alkove_isolator import LEKO_SUPPORT_DIR, VOICEPRINT_PATH, max_chunk_rms

HISTORY_PATH = LEKO_SUPPORT_DIR / "voiceprint-history.json"
MAX_HISTORY = 20
MIN_CLUSTER_COUNT = 4
MIN_EMBED_SAMPLES = 24_000
MAX_SHORT_BUFFER = 48_000
MIN_EMBED_RMS = 0.002


def _atomic_write_json(path: Path, payload: dict) -> None:
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_name(f".{path.name}.tmp")
    tmp.write_text(json.dumps(payload), encoding="utf-8")
    os.replace(tmp, path)


def _read_json(path: Path) -> Optional[dict]:
    try:
        if not path.exists():
            return None
        data = json.loads(path.read_text(encoding="utf-8"))
        return data if isinstance(data, dict) else None
    except (OSError, json.JSONDecodeError, ValueError):
        return None


def _extract_vp_dict(payload: Optional[dict]) -> Optional[dict]:
    if not isinstance(payload, dict):
        return None
    inner = payload.get("voiceprint")
    if isinstance(inner, dict):
        return inner
    return payload if payload else None


def dominant_cluster_entry(
    entries: List[dict],
    verify_fn: Callable[[dict, dict], Tuple[bool, float]],
    min_count: int = MIN_CLUSTER_COUNT,
) -> Tuple[Optional[dict], int]:
    """Return `(medoid_entry, count)` for the strict-majority voice in `entries`."""
    total = len(entries)
    if total < min_count:
        return None, 0
    best_idx = -1
    best_count = 0
    best_score = -1.0
    for i, cand in enumerate(entries):
        cand_vp = _extract_vp_dict(cand)
        if cand_vp is None:
            continue
        matches = 0
        score_sum = 0.0
        for j, other in enumerate(entries):
            other_vp = _extract_vp_dict(other)
            if other_vp is None:
                continue
            same, score = (True, 1.0) if i == j else verify_fn(cand_vp, other_vp)
            if same:
                matches += 1
                score_sum += float(score)
        if (matches, score_sum, i) > (best_count, best_score, best_idx):
            best_idx = i
            best_count = matches
            best_score = score_sum
    if best_idx < 0 or best_count < min_count or best_count * 2 <= total:
        return None, 0
    return entries[best_idx], best_count


class RollingVoiceprintStore:
    """Maintains a bounded rolling window of recent turn embeddings without saving audio."""

    def __init__(
        self,
        history_path: Path = HISTORY_PATH,
        voiceprint_path: Path = VOICEPRINT_PATH,
        max_history: int = MAX_HISTORY,
        min_cluster_count: int = MIN_CLUSTER_COUNT,
        embed_fn: Optional[Callable[[np.ndarray], dict]] = None,
        verify_fn: Optional[Callable[[dict, dict], Tuple[bool, float]]] = None,
    ):
        self.history_path = Path(history_path)
        self.voiceprint_path = Path(voiceprint_path)
        self.max_history = max(4, int(max_history))
        self.min_cluster_count = max(2, int(min_cluster_count))
        self._embed_fn = embed_fn
        self._verify_fn = verify_fn
        self._lock = threading.Lock()
        self._alkove = None
        self._timbre = None
        self._failed = False
        self._short_buf = None

    def _ensure_alkove(self) -> bool:
        if self._embed_fn is not None and self._verify_fn is not None:
            return True
        if self._failed:
            return False
        with self._lock:
            if self._timbre is not None:
                return True
            try:
                import alkove

                models = alkove.Models.load(["timbre"])
                self._alkove = alkove
                self._timbre = alkove.Timbre(models)
                return True
            except Exception:
                self._failed = True
                return False

    def _embed(self, audio_f32: np.ndarray) -> Optional[dict]:
        if self._embed_fn is not None:
            return self._embed_fn(audio_f32)
        if not self._ensure_alkove():
            return None
        vp = self._timbre.embed(audio_f32)
        data = vp.to_json()
        return data if isinstance(data, dict) else json.loads(data)

    def _verify(self, vp_a: dict, vp_b: dict) -> Tuple[bool, float]:
        if self._verify_fn is not None:
            return self._verify_fn(vp_a, vp_b)
        if not self._ensure_alkove():
            return False, 0.0
        a = self._alkove.Voiceprint.from_json(vp_a)
        b = self._alkove.Voiceprint.from_json(vp_b)
        return self._alkove.Timbre.verify(a, b, level="balanced")

    def _collect_clip(self, audio_f32: np.ndarray) -> Optional[np.ndarray]:
        audio = np.asarray(audio_f32, dtype=np.float32).reshape(-1)
        if not audio.size or max_chunk_rms(audio) < MIN_EMBED_RMS:
            return None
        with self._lock:
            if len(audio) >= MIN_EMBED_SAMPLES:
                self._short_buf = None
                return audio
            combined = audio if self._short_buf is None else np.concatenate((self._short_buf, audio))
            if len(combined) > MAX_SHORT_BUFFER:
                combined = combined[-MAX_SHORT_BUFFER:]
            if len(combined) < MIN_EMBED_SAMPLES:
                self._short_buf = combined
                return None
            self._short_buf = None
            return combined

    def record_turn(self, audio_f32: np.ndarray, transcript: str = "") -> bool:
        """Record one accepted turn's embedding and promote majority voiceprint if ready."""
        clip = self._collect_clip(audio_f32)
        if clip is None:
            return False
        vp_dict = self._embed(clip)
        if not isinstance(vp_dict, dict):
            return False

        with self._lock:
            raw = _read_json(self.history_path) or {}
            items = list(raw.get("entries") or [])
            window = deque(items, maxlen=self.max_history)
            window.append({
                "at": round(time.time(), 3),
                "chars": len((transcript or "").strip()),
                "voiceprint": vp_dict,
            })
            entries = list(window)
            _atomic_write_json(self.history_path, {"version": 1, "entries": entries})

            existing = _read_json(self.voiceprint_path)
            if isinstance(existing, dict) and existing.get("source") == "manual":
                return False

            winner, count = dominant_cluster_entry(entries, self._verify, self.min_cluster_count)
            if winner is None:
                return False
            winner_vp = winner["voiceprint"]
            existing_vp = _extract_vp_dict(existing)
            if existing_vp is not None:
                same, _ = self._verify(existing_vp, winner_vp)
                prev_count = existing.get("count", 0) if isinstance(existing, dict) else 0
                if same and count <= prev_count and count not in (4, 8, 12, 16, 20):
                    return False

            _atomic_write_json(self.voiceprint_path, {
                "source": "auto",
                "count": count,
                "window": len(entries),
                "voiceprint": winner_vp,
            })
            return True


_DEFAULT_STORE: Optional[RollingVoiceprintStore] = None
_QUEUE: Optional[queue.Queue] = None
_WORKER: Optional[threading.Thread] = None
_WORKER_LOCK = threading.Lock()


def _run_worker(q: queue.Queue) -> None:
    global _DEFAULT_STORE
    while True:
        item = q.get()
        try:
            audio, text = item
            if _DEFAULT_STORE is None:
                _DEFAULT_STORE = RollingVoiceprintStore()
            _DEFAULT_STORE.record_turn(audio, text)
        except Exception:
            pass
        finally:
            q.task_done()


def record_auto_voiceprint(audio_f32, transcript: str = "") -> None:
    """Queue an accepted turn asynchronously so STT->Yellow latency stays unchanged."""
    global _QUEUE, _WORKER
    with _WORKER_LOCK:
        if _QUEUE is None:
            _QUEUE = queue.Queue(maxsize=4)
            _WORKER = threading.Thread(target=_run_worker, args=(_QUEUE,), name="rolling-voiceprint", daemon=True)
            _WORKER.start()
    try:
        clip = audio_f32.copy() if hasattr(audio_f32, "copy") else audio_f32
        _QUEUE.put_nowait((clip, transcript))
    except queue.Full:
        pass
