"""Localhost playground server for tuning reply-card word/letter timing against TTS audio."""
from __future__ import annotations

import argparse
import base64
import io
import json
import re
import threading
import wave
from http import HTTPStatus
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from typing import Any, Callable, Dict, Optional

import numpy as np

from ..normalization.verbalizer import verbalize_for_speech
from .ui_bridge import CARD_TIMING_PATH, DEFAULT_CARD_TIMING, compute_word_timings_ms, load_card_timing

REPO_ROOT = Path(__file__).resolve().parents[2]
WEB_DIR = REPO_ROOT / "ui" / "src" / "web"
PRESETS = [
    "No active countdowns.",
    "I can show active countdowns, rules, or passcodes.",
    "The Technology Management Rule was saved.",
    "You have two active countdowns: Study Time Limit, 5 minutes remaining, and Notes Timed Pass, 8 minutes remaining.",
]
STATIC_TYPES = {
    "/": ("tuner.html", "text/html; charset=utf-8"),
    "/tuner.html": ("tuner.html", "text/html; charset=utf-8"),
    "/tuner.js": ("tuner.js", "application/javascript; charset=utf-8"),
    "/pill.js": ("pill.js", "application/javascript; charset=utf-8"),
    "/pill.css": ("pill.css", "text/css; charset=utf-8"),
    "/card.css": ("card.css", "text/css; charset=utf-8"),
    "/tokens.css": ("tokens.css", "text/css; charset=utf-8"),
}


def save_card_timing(data: Dict[str, Any], path: Optional[Path] = None) -> Dict[str, Any]:
    cfg = dict(DEFAULT_CARD_TIMING)
    if data.get("reveal_mode") in ("word", "letter"):
        cfg["reveal_mode"] = data["reveal_mode"]
    if isinstance(data.get("lead_ms"), (int, float)):
        cfg["lead_ms"] = max(-250, min(250, int(round(data["lead_ms"]))))
    if isinstance(data.get("gap_bridge_ms"), (int, float)):
        cfg["gap_bridge_ms"] = max(20, min(400, int(round(data["gap_bridge_ms"]))))
    if isinstance(data.get("end_pad_ms"), (int, float)):
        cfg["end_pad_ms"] = max(-300, min(300, int(round(data["end_pad_ms"]))))
    if isinstance(data.get("energy_threshold"), (int, float)):
        cfg["energy_threshold"] = round(max(0.005, min(0.25, float(data["energy_threshold"]))), 4)
    target = path or CARD_TIMING_PATH
    target.parent.mkdir(parents=True, exist_ok=True)
    tmp = target.with_suffix(".tmp")
    tmp.write_text(json.dumps(cfg, indent=2) + "\n", encoding="utf-8")
    tmp.replace(target)
    return cfg


def pcm_to_wav_bytes(pcm, sample_rate: int) -> bytes:
    arr = np.asarray(pcm, dtype=np.float32).reshape(-1)
    clipped = np.clip(arr, -1.0, 1.0)
    int16 = (clipped * 32767.0).astype(np.int16)
    buf = io.BytesIO()
    with wave.open(buf, "wb") as wf:
        wf.setnchannels(1)
        wf.setsampwidth(2)
        wf.setframerate(int(sample_rate))
        wf.writeframes(int16.tobytes())
    return buf.getvalue()


def _fallback_speech_pcm(text: str, sample_rate: int = 24000) -> np.ndarray:
    clauses = [c for c in re.findall(r"[^,.;:!?]+[,.;:!?]*\s*", text) if c.strip()] or [text]
    parts = [np.zeros(int(sample_rate * 0.08), dtype=np.float32)]
    for idx, clause in enumerate(clauses):
        n_words = max(1, len(re.findall(r"\S+", clause)))
        dur = max(0.32, n_words * 0.24)
        t = np.linspace(0.0, dur, int(sample_rate * dur), endpoint=False, dtype=np.float32)
        env = 0.25 * (0.65 + 0.35 * np.sin(2 * np.pi * (n_words / dur) * t))
        tone = env * (0.6 * np.sin(2 * np.pi * 195 * t) + 0.4 * np.sin(2 * np.pi * 390 * t))
        parts.append(tone.astype(np.float32))
        if idx + 1 < len(clauses):
            parts.append(np.zeros(int(sample_rate * 0.22), dtype=np.float32))
    parts.append(np.zeros(int(sample_rate * 0.24), dtype=np.float32))
    return np.concatenate(parts)


def _default_synth(spoken: str) -> tuple[np.ndarray, int, str]:
    try:
        from . import assistant
        tts = assistant.get_tts()
        pcm = tts.synth(spoken, voice=assistant.VOICE_NAME)
        sr = int(getattr(tts, "sample_rate", 24000) or 24000)
        return np.asarray(pcm, dtype=np.float32).reshape(-1), sr, "ptts"
    except Exception:
        return _fallback_speech_pcm(spoken), 24000, "fallback"


class TunerState:
    def __init__(self, synth_fn: Optional[Callable[[str], tuple[Any, int, str]]] = None,
                 config_path: Optional[Path] = None):
        self._synth_fn = synth_fn or _default_synth
        self.config_path = config_path or CARD_TIMING_PATH
        self._cache: Dict[str, tuple[np.ndarray, int, str, bytes]] = {}
        self._lock = threading.Lock()

    def get_audio(self, spoken: str) -> tuple[np.ndarray, int, str, bytes]:
        with self._lock:
            cached = self._cache.get(spoken)
        if cached is not None:
            return cached
        pcm, sr, engine = self._synth_fn(spoken)
        arr = np.asarray(pcm, dtype=np.float32).reshape(-1)
        wav_bytes = pcm_to_wav_bytes(arr, sr)
        entry = (arr, int(sr), str(engine), wav_bytes)
        with self._lock:
            self._cache[spoken] = entry
        return entry

    def analyze(self, payload: Dict[str, Any]) -> Dict[str, Any]:
        text = str(payload.get("text") or "").strip() or PRESETS[0]
        spoken = verbalize_for_speech(text) or text
        arr, sr, engine, wav_bytes = self.get_audio(spoken)
        cfg = load_card_timing(self.config_path)
        lead_ms = int(round(payload["lead_ms"])) if isinstance(payload.get("lead_ms"), (int, float)) else cfg["lead_ms"]
        threshold = float(payload["energy_threshold"]) if isinstance(payload.get("energy_threshold"), (int, float)) else cfg["energy_threshold"]
        gap_ms = int(round(payload["gap_bridge_ms"])) if isinstance(payload.get("gap_bridge_ms"), (int, float)) else cfg["gap_bridge_ms"]
        end_pad = int(round(payload["end_pad_ms"])) if isinstance(payload.get("end_pad_ms"), (int, float)) else cfg["end_pad_ms"]
        reveal_mode = payload.get("reveal_mode") if payload.get("reveal_mode") in ("word", "letter") else cfg["reveal_mode"]
        details = compute_word_timings_ms(
            arr, sr, text, lead_ms=lead_ms, energy_threshold=threshold,
            gap_bridge_ms=gap_ms, end_pad_ms=end_pad, return_details=True,
        )
        words = [w.strip() for w in re.findall(r"\S+\s*", text)]
        result: Dict[str, Any] = {
            "text": text,
            "spoken": spoken,
            "words": words,
            "reveal_mode": reveal_mode,
            "sample_rate": sr,
            "engine": engine,
            **details,
        }
        if payload.get("include_audio", True):
            result["audio_b64"] = base64.b64encode(wav_bytes).decode("ascii")
        return result


def create_server(host: str = "127.0.0.1", port: int = 8787,
                  state: Optional[TunerState] = None,
                  config_path: Optional[Path] = None) -> ThreadingHTTPServer:
    tuner = state or TunerState(config_path=config_path)

    class Handler(BaseHTTPRequestHandler):
        def log_message(self, format: str, *args: Any) -> None:  # noqa: A002
            return

        def _send_json(self, code: int, data: Dict[str, Any]) -> None:
            body = json.dumps(data).encode("utf-8")
            self.send_response(code)
            self.send_header("Content-Type", "application/json; charset=utf-8")
            self.send_header("Content-Length", str(len(body)))
            self.send_header("Cache-Control", "no-store")
            self.end_headers()
            self.wfile.write(body)

        def _read_json(self) -> Dict[str, Any]:
            length = int(self.headers.get("Content-Length") or "0")
            raw = self.rfile.read(length) if length > 0 else b"{}"
            data = json.loads(raw.decode("utf-8") or "{}")
            return data if isinstance(data, dict) else {}

        def do_GET(self) -> None:  # noqa: N802
            path = self.path.split("?", 1)[0]
            if path == "/api/config":
                self._send_json(HTTPStatus.OK, {
                    "config": load_card_timing(tuner.config_path),
                    "defaults": DEFAULT_CARD_TIMING,
                    "presets": PRESETS,
                })
                return
            static = STATIC_TYPES.get(path)
            if static:
                filename, ctype = static
                target = WEB_DIR / filename
                if target.exists():
                    body = target.read_bytes()
                    self.send_response(HTTPStatus.OK)
                    self.send_header("Content-Type", ctype)
                    self.send_header("Content-Length", str(len(body)))
                    self.send_header("Cache-Control", "no-store")
                    self.end_headers()
                    self.wfile.write(body)
                    return
            self._send_json(HTTPStatus.NOT_FOUND, {"error": "not_found"})

        def do_POST(self) -> None:  # noqa: N802
            path = self.path.split("?", 1)[0]
            try:
                body = self._read_json()
            except (UnicodeDecodeError, json.JSONDecodeError):
                self._send_json(HTTPStatus.BAD_REQUEST, {"error": "invalid_json"})
                return
            if path == "/api/config":
                saved = save_card_timing(body, tuner.config_path)
                self._send_json(HTTPStatus.OK, {"ok": True, "config": saved})
                return
            if path == "/api/synth":
                self._send_json(HTTPStatus.OK, tuner.analyze(body))
                return
            self._send_json(HTTPStatus.NOT_FOUND, {"error": "not_found"})

    return ThreadingHTTPServer((host, port), Handler)


def main(argv: Optional[list[str]] = None) -> int:
    parser = argparse.ArgumentParser(description="Run the Leko speech/card timing tuner localhost server.")
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, default=8787)
    parser.add_argument("--no-warm", action="store_true")
    args = parser.parse_args(argv)
    tuner = TunerState()
    if not args.no_warm:
        threading.Thread(target=lambda: tuner.get_audio(PRESETS[0]), daemon=True).start()
    server = create_server(args.host, args.port, state=tuner)
    print(f"Leko Timing Tuner running at http://{args.host}:{server.server_address[1]}", flush=True)
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass
    finally:
        server.server_close()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
