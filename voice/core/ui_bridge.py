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
CARD_TIMING_PATH = Path.home() / "Library/Application Support/Leko/card-timing.json"
DEFAULT_CARD_TIMING: Dict[str, Any] = {
    "reveal_mode": "word",
    "lead_ms": -30,
    "energy_threshold": 0.04,
    "gap_bridge_ms": 120,
    "end_pad_ms": 0,
}


def load_card_timing(path: Optional[Path] = None) -> Dict[str, Any]:
    cfg = dict(DEFAULT_CARD_TIMING)
    try:
        raw = json.loads((path or CARD_TIMING_PATH).read_text(encoding="utf-8"))
        if isinstance(raw, dict):
            if raw.get("reveal_mode") in ("word", "letter"):
                cfg["reveal_mode"] = raw["reveal_mode"]
            for key in ("lead_ms", "gap_bridge_ms", "end_pad_ms"):
                if isinstance(raw.get(key), (int, float)):
                    cfg[key] = int(round(raw[key]))
            if isinstance(raw.get("energy_threshold"), (int, float)):
                cfg["energy_threshold"] = max(0.005, min(0.25, float(raw["energy_threshold"])))
    except (OSError, json.JSONDecodeError, ValueError):
        pass
    return cfg


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

    def emit_reply_card(self, text: str, columns: Optional[list] = None,
                        rows: Optional[list] = None) -> None:
        payload: Dict[str, Any] = {"type": "reply_card", "text": text}
        if columns:
            payload["columns"] = list(columns)
        if rows:
            payload["rows"] = list(rows)
        self.emit(payload)

    def emit_speaking(self, text: str, duration_ms: Optional[int] = None,
                      word_timings_ms: Optional[list] = None,
                      reveal_mode: Optional[str] = None) -> None:
        payload: Dict[str, Any] = {
            "type": "state",
            "state": "speaking",
            "text": text,
            "label": "Maeve Speaking...",
        }
        if duration_ms is not None and duration_ms > 0:
            payload["duration_ms"] = int(duration_ms)
        if word_timings_ms:
            payload["word_timings_ms"] = [int(t) for t in word_timings_ms]
        if reveal_mode in ("word", "letter"):
            payload["reveal_mode"] = reveal_mode
        self.emit(payload)

    def emit_audio_level(self, level: float) -> None:
        clamped = max(0.0, min(1.0, float(level)))
        self.emit({"type": "audio_level", "level": round(clamped, 3)})

    def emit_rms_level(self, rms: float, ceiling: float = 0.03, min_interval: float = 0.035,
                       floor: float = 0.003, gamma: float = 0.48) -> None:
        import time
        level = 0.0 if rms <= floor else min(1.0, max(0.0, (rms - floor) / ceiling) ** gamma)
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


def _word_weight(word: str) -> float:
    import re
    clean = re.sub(r"[^a-zA-Z0-9]", "", word)
    syllables = len(re.findall(r"[aeiouy]+", clean.lower())) or 1
    digits = sum(ch.isdigit() for ch in clean)
    return 0.4 + 0.45 * len(clean) + 0.8 * syllables + 1.5 * digits


def compute_word_timings_ms(pcm, sample_rate: int, text: str, lead_ms: Optional[int] = None,
                            energy_threshold: Optional[float] = None, gap_bridge_ms: Optional[int] = None,
                            end_pad_ms: Optional[int] = None, return_details: bool = False):
    """Compute per-word reveal offsets (ms) from synthesized PCM energy and clauses."""
    import re
    import numpy as np
    cfg = load_card_timing()
    lead = int(cfg["lead_ms"] if lead_ms is None else lead_ms)
    ratio = float(cfg["energy_threshold"] if energy_threshold is None else energy_threshold)
    gap_frames = max(1, int(round((cfg["gap_bridge_ms"] if gap_bridge_ms is None else gap_bridge_ms) / 20)))
    pad_end = int(cfg["end_pad_ms"] if end_pad_ms is None else end_pad_ms)
    words = re.findall(r"\S+\s*", text or "")
    arr = np.asarray(pcm, dtype=np.float32).reshape(-1)
    total_ms = int(round(len(arr) * 1000 / sample_rate)) if sample_rate else 0
    if not words or not sample_rate:
        return {"timings_ms": [], "segments_ms": [], "duration_ms": total_ms, "end_ms": total_ms, "rms_20ms": [], "threshold": 0.0} if return_details else []
    win = max(1, int(sample_rate * 0.02))
    n_win = len(arr) // win
    if n_win <= 0:
        timings = [0 for _ in words]
        return {"timings_ms": timings, "segments_ms": [], "duration_ms": total_ms, "end_ms": total_ms, "rms_20ms": [], "threshold": 0.0} if return_details else timings
    rms = np.sqrt(np.mean(arr[:n_win * win].reshape(n_win, win) ** 2, axis=1))
    thresh = max(float(rms.max()) * ratio, 1e-4)
    active = [bool(v > thresh) for v in rms]
    if not any(active):
        timings = [max(0, int(round(i * total_ms / max(1, len(words)))) + lead) for i in range(len(words))]
        return {"timings_ms": timings, "segments_ms": [], "duration_ms": total_ms, "end_ms": max(0, total_ms + lead + pad_end), "rms_20ms": [round(float(v), 5) for v in rms], "threshold": round(thresh, 5)} if return_details else timings
    bridged = list(active)
    idx = 0
    while idx < len(bridged):
        if not bridged[idx]:
            end = idx
            while end < len(bridged) and not bridged[end]:
                end += 1
            if 0 < idx and end < len(bridged) and (end - idx) < gap_frames:
                for pos in range(idx, end):
                    bridged[pos] = True
            idx = end
        else:
            idx += 1
    segs, start = [], None
    for pos, voiced in enumerate(bridged):
        if voiced and start is None:
            start = pos * 20
        elif not voiced and start is not None:
            segs.append((start, pos * 20))
            start = None
    if start is not None:
        segs.append((start, len(bridged) * 20))
    clauses = [c for c in re.findall(r"[^,.;:!?]+[,.;:!?]*\s*", text) if c.strip()]
    timings: list[int] = []
    if len(segs) == len(clauses) and clauses:
        for (seg_a, seg_b), clause in zip(segs, clauses):
            c_words = re.findall(r"\S+\s*", clause)
            weights = [_word_weight(w) for w in c_words]
            total = sum(weights) or 1.0
            span = max(60, (seg_b - seg_a) + pad_end)
            acc = 0.0
            for wt in weights:
                timings.append(max(0, int(round(seg_a + (acc / total) * span)) + lead))
                acc += wt
    else:
        voiced_ms = [pos * 20 for pos, v in enumerate(bridged) if v] or [segs[0][0]]
        weights = [_word_weight(w) + (2.4 if re.search(r"[.!?]$", w.strip()) else 1.2 if re.search(r"[,;:]$", w.strip()) else 0.0) for w in words]
        total = sum(weights) or 1.0
        acc = 0.0
        for wt in weights:
            frame = min(len(voiced_ms) - 1, int((acc / total) * len(voiced_ms)))
            timings.append(max(0, int(voiced_ms[frame]) + lead))
            acc += wt
    end_ms = max((timings[-1] + 120) if timings else 0, segs[-1][1] + lead + pad_end)
    if return_details:
        return {"timings_ms": timings, "segments_ms": segs, "duration_ms": total_ms, "end_ms": end_ms, "rms_20ms": [round(float(v), 5) for v in rms], "threshold": round(thresh, 5)}
    return timings
