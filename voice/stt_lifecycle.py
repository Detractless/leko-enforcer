"""STT model lifecycle: lazy load, page-cache warm, and idle unload.

Ported from DWB Dictation's warm-cache + idle-unload pattern.
"""
from __future__ import annotations

import os
import threading
import time
from pathlib import Path
from typing import Any, Callable, Optional


class SttLifecycle:
    """Manages an STT model's lifecycle: lazy load, warm cache, idle unload."""

    def __init__(
        self,
        model_path: Path,
        loader: Callable[[Path], Any],
        idle_timeout_s: float = 300.0,
        clock: Callable[[], float] = time.monotonic,
    ):
        self.model_path = Path(model_path)
        self.loader = loader
        self.idle_timeout_s = idle_timeout_s
        self._clock = clock
        self._model: Optional[Any] = None
        self._last_used: float = 0.0
        self._lock = threading.Lock()
        self._transcribing = False

    def is_loaded(self) -> bool:
        return self._model is not None

    def get_model(self) -> Any:
        """Lazy-load the model on first call; update last-used timestamp."""
        with self._lock:
            if self._model is None:
                self._model = self.loader(self.model_path)
            self._last_used = self._clock()
            return self._model

    def mark_used(self) -> None:
        with self._lock:
            self._last_used = self._clock()

    def set_transcribing(self, active: bool) -> None:
        with self._lock:
            self._transcribing = active
            if active:
                self._last_used = self._clock()

    def warm_cache(self) -> bool:
        """Read model files into OS page cache without holding them in process memory.

        Returns True on success, False if the path is missing or unreadable.
        """
        paths: list[Path] = []
        if self.model_path.is_dir():
            for root, _dirs, files in os.walk(self.model_path):
                for name in files:
                    paths.append(Path(root) / name)
        elif self.model_path.is_file():
            paths.append(self.model_path)
        else:
            return False

        try:
            for p in paths:
                with open(p, "rb") as f:
                    while f.read(1024 * 1024):
                        pass
            return True
        except OSError:
            return False

    def unload_if_idle(self) -> bool:
        """Unload the model if idle longer than timeout. Returns True if unloaded.

        Deferred if a transcription is in progress.
        """
        with self._lock:
            if self._model is None:
                return False
            if self._transcribing:
                return False
            if self._last_used <= 0:
                return False
            idle = self._clock() - self._last_used
            if idle >= self.idle_timeout_s:
                self._model = None
                self._last_used = 0.0
                return True
            return False

    def unload(self) -> None:
        with self._lock:
            self._model = None
            self._last_used = 0.0

    def idle_seconds(self) -> float:
        with self._lock:
            if self._last_used <= 0:
                return 0.0
            return self._clock() - self._last_used