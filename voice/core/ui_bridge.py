"""UI IPC Bridge for Leko Voice Assistant.
Translates voice pipeline states into JSON-line IPC events for the floating pill.
"""
from __future__ import annotations

import json
import subprocess
import sys
import threading
from pathlib import Path
from typing import Callable, Optional, Dict, Any

REPO_ROOT = Path(__file__).resolve().parents[2]
LEKO_UI_DEBUG = REPO_ROOT / "target/debug/leko-pill"
LEKO_UI_RELEASE = REPO_ROOT / "target/release/leko-pill"
PILL_ACTIONS = Path.home() / "Library/Application Support/Leko/pill-actions.json"

class UIPipelineBridge:
    def __init__(self, event_sink: Optional[Callable[[Dict[str, Any]], None]] = None):
        self._custom_sink = event_sink
        self._proc: Optional[subprocess.Popen] = None
        self._on_action_callback: Optional[Callable[[Dict[str, Any]], None]] = None
        self._reader_thread: Optional[threading.Thread] = None

    def emit(self, event: Dict[str, Any]) -> None:
        if self._custom_sink is not None:
            self._custom_sink(event)
            return

        line = json.dumps(event) + "\n"
        if self._proc and self._proc.stdin:
            try:
                self._proc.stdin.write(line)
                self._proc.stdin.flush()
            except BrokenPipeError:
                pass
        else:
            # Fallback to stdout
            sys.stdout.write(line)
            sys.stdout.flush()

    def emit_state(self, state: str, label: Optional[str] = None) -> None:
        payload: Dict[str, Any] = {"type": "state", "state": state}
        if label:
            payload["label"] = label
        self.emit(payload)

    def emit_listening(self) -> None:
        self.emit_state("listening")

    def emit_thinking(self, query: Optional[str] = None) -> None:
        payload: Dict[str, Any] = {"type": "state", "state": "thinking"}
        if query:
            payload["query"] = query
        self.emit(payload)

    def emit_draft(self, name: str, kind: str, description: str, raw_intent: Optional[Dict[str, Any]] = None, action_id: Optional[str] = None) -> None:
        self.emit({
            "type": "draft",
            "draft": {
                "id": action_id,
                "name": name,
                "kind": kind,
                "description": description,
                "intent": raw_intent or {}
            }
        })

    def emit_speaking(self, text: str) -> None:
        self.emit({
            "type": "state",
            "state": "speaking",
            "text": text,
            "label": "Maeve Speaking..."
        })

    def emit_audio_level(self, level: float) -> None:
        clamped = max(0.0, min(1.0, float(level)))
        self.emit({"type": "audio_level", "level": round(clamped, 3)})

    def emit_rms_level(self, rms: float, ceiling: float = 0.10, min_interval: float = 0.035) -> None:
        import time
        level = 0.0 if rms <= 0.004 else min(1.0, max(0.0, (rms - 0.004) / ceiling) ** 0.6)
        last_val = getattr(self, "_last_level_val", 0.0)
        if level == 0.0:
            if last_val == 0.0:
                return
            self._last_level_val = 0.0
            self.emit_audio_level(0.0)
            return
        now = time.monotonic()
        last = getattr(self, "_last_level_emit", 0.0)
        if last_val > 0.0 and now - last < min_interval:
            return
        self._last_level_emit = now
        self._last_level_val = level
        self.emit_audio_level(level)

    def emit_confirmed(self, label: str = "Locked & Saved") -> None:
        self.emit_state("confirmed", label)

    def emit_idle(self) -> None:
        self.emit_state("idle", "Ready")

    def start_ui_process(self, on_action: Optional[Callable[[Dict[str, Any]], None]] = None) -> None:
        """Launches the native leko-pill panel and watches its action file."""
        ui_bin = LEKO_UI_RELEASE if LEKO_UI_RELEASE.exists() else LEKO_UI_DEBUG
        if not ui_bin.exists():
            raise FileNotFoundError(f"leko-pill binary not found at {LEKO_UI_DEBUG}")

        self._on_action_callback = on_action
        self._proc = subprocess.Popen(
            [str(ui_bin)],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            bufsize=1
        )

        def _read_loop():
            if not self._proc or not self._proc.stdout:
                return
            for line in self._proc.stdout:
                line = line.strip()
                if not line:
                    continue
                try:
                    data = json.loads(line)
                    if self._on_action_callback:
                        self._on_action_callback(data)
                except json.JSONDecodeError:
                    pass
            if self._on_action_callback:
                self._on_action_callback({"action": "window_closed"})

        self._reader_thread = threading.Thread(target=_read_loop, daemon=True)
        self._reader_thread.start()

        def _watch_actions():
            last = ""
            while self._proc is not None:
                try:
                    if not PILL_ACTIONS.exists():
                        threading.Event().wait(0.1)
                        continue
                    data = json.loads(PILL_ACTIONS.read_text(encoding="utf-8"))
                    action = data.get("action", "")
                    if action and action != last and self._on_action_callback:
                        last = action
                        self._on_action_callback({"action": action})
                except (OSError, json.JSONDecodeError):
                    pass
                threading.Event().wait(0.1)

        if not self._proc.stdout:
            self._action_thread = threading.Thread(target=_watch_actions, daemon=True)
            self._action_thread.start()

    def close(self) -> None:
        if self._proc:
            try:
                self._proc.terminate()
            except Exception:
                pass
            self._proc = None


def start_playback_sampler(pcm, sample_rate: int, on_audio_level: Optional[Callable[[float], None]]) -> Callable[[], None]:
    if not on_audio_level or not sample_rate:
        return lambda: None
    import time
    import numpy as np
    arr = np.asarray(pcm, dtype=np.float32).reshape(-1)
    if not len(arr):
        return lambda: None
    stop_event = threading.Event()
    win = max(1, int(sample_rate * 0.04))
    started = time.monotonic()

    def _run():
        while not stop_event.wait(0.04):
            idx = min(len(arr), int((time.monotonic() - started) * sample_rate))
            start = max(0, idx - win)
            if idx > start:
                sl = arr[start:idx]
                try:
                    on_audio_level(float(np.sqrt(np.mean(sl * sl))))
                except Exception:
                    pass
            if idx >= len(arr):
                break

    threading.Thread(target=_run, daemon=True, name="tts-level-meter").start()

    def _stop():
        stop_event.set()
        try:
            on_audio_level(0.0)
        except Exception:
            pass

    return _stop
