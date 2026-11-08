"""UI IPC Bridge for Leko Voice Assistant.
Translates voice pipeline states and drafts into JSON-line IPC events for the minimalist circle UI.
"""
from __future__ import annotations

import json
import os
import subprocess
import sys
import threading
from pathlib import Path
from typing import Callable, Optional, Dict, Any

REPO_ROOT = Path(__file__).resolve().parent.parent
LEKO_UI_DEBUG = REPO_ROOT / "target/debug/leko-ui"
LEKO_UI_RELEASE = REPO_ROOT / "target/release/leko-ui"

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

    def emit_confirmed(self, label: str = "Locked & Saved") -> None:
        self.emit_state("confirmed", label)

    def emit_idle(self) -> None:
        self.emit_state("idle", "Ready")

    def start_ui_process(self, on_action: Optional[Callable[[Dict[str, Any]], None]] = None) -> None:
        """Launches native leko-ui WebKit window in background and opens stdio pipe."""
        ui_bin = LEKO_UI_RELEASE if LEKO_UI_RELEASE.exists() else LEKO_UI_DEBUG
        if not ui_bin.exists():
            raise FileNotFoundError(f"leko-ui binary not found at {LEKO_UI_DEBUG}")

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

    def close(self) -> None:
        if self._proc:
            try:
                self._proc.terminate()
            except Exception:
                pass
            self._proc = None
