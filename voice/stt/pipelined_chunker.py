"""Pipelined chunking: transcribe earlier audio windows while recording continues.

Ported from DWB Dictation's prechunk worker pattern.
"""
from __future__ import annotations

import threading
from typing import Callable, List, Optional


class PipelinedChunker:
    """Chunks long audio and transcribes completed windows in the background."""

    def __init__(
        self,
        transcribe_fn: Callable[[list], str],
        chunk_window_s: float = 30.0,
        sample_rate: int = 16000,
        on_intermediate: Optional[Callable[[str], None]] = None,
    ):
        self.transcribe_fn = transcribe_fn
        self.chunk_window_s = chunk_window_s
        self.sample_rate = sample_rate
        self.on_intermediate = on_intermediate
        self._chunk_size = int(chunk_window_s * sample_rate)
        self._windows: List[list] = []
        self._results: List[str] = []
        self._current: list = []
        self._lock = threading.Lock()
        self._cancelled = False

    def feed(self, samples: list) -> None:
        """Feed audio samples; transcribe completed windows."""
        if self._cancelled:
            return
        with self._lock:
            self._current.extend(samples)
            while len(self._current) >= self._chunk_size:
                window = self._current[: self._chunk_size]
                self._current = self._current[self._chunk_size:]
                self._windows.append(window)
                text = self.transcribe_fn(window)
                self._results.append(text)
                if self.on_intermediate is not None:
                    self.on_intermediate(text)

    def finalize(self) -> str:
        """Transcribe any remaining audio and return the full transcript."""
        if self._cancelled:
            self.reset()
            return ""
        with self._lock:
            if self._current:
                text = self.transcribe_fn(self._current)
                self._results.append(text)
                self._current = []
        return " ".join(r for r in self._results if r).strip()

    def cancel(self) -> None:
        """Cancel and discard all partial results."""
        with self._lock:
            self._cancelled = True
            self._windows.clear()
            self._results.clear()
            self._current.clear()

    def reset(self) -> None:
        """Reset state for reuse."""
        with self._lock:
            self._cancelled = False
            self._windows.clear()
            self._results.clear()
            self._current.clear()

    @property
    def partial_results(self) -> List[str]:
        with self._lock:
            return list(self._results)