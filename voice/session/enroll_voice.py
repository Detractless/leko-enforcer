"""Enroll or clear the user's Alkove Timbre voiceprint for speaker isolation."""
from __future__ import annotations

import argparse
import json
import sys
import time
from pathlib import Path
from typing import Optional

import numpy as np

from ..core.alkove_isolator import LEKO_SUPPORT_DIR, VOICEPRINT_PATH, max_chunk_rms


def enroll_from_pcm(audio_f32: np.ndarray, out_path: Path = VOICEPRINT_PATH) -> Path:
    """Compute a Timbre voiceprint from 16kHz mono float32 audio and persist it."""
    import alkove

    audio = np.asarray(audio_f32, dtype=np.float32).reshape(-1)
    if len(audio) < 24_000:
        raise ValueError("Please speak for at least 1.5 seconds so Timbre can capture your voice.")
    if max_chunk_rms(audio) < 0.002:
        raise ValueError("Audio level too low; speak at normal volume closer to your mic.")
    models = alkove.Models.load(["timbre"])
    timbre = alkove.Timbre(models)
    vp = timbre.embed(audio)
    out_path = Path(out_path)
    out_path.parent.mkdir(parents=True, exist_ok=True)
    data = vp.to_json()
    out_path.write_text(data if isinstance(data, str) else json.dumps(data), encoding="utf-8")
    return out_path


def record_clip(seconds: float = 3.5, sample_rate: int = 16000) -> np.ndarray:
    """Record a mono 16kHz clip from the resolved input microphone."""
    import sounddevice as sd
    from ..core.stream_recovery import open_input_stream

    chunks = []
    done = time.monotonic() + max(2.0, float(seconds))
    stream = open_input_stream(
        sd,
        samplerate=sample_rate,
        channels=1,
        dtype="int16",
        blocksize=1280,
        callback=lambda indata, _f, _t, _s: chunks.append(np.frombuffer(bytes(indata), dtype=np.int16).copy()),
    )
    try:
        stream.start()
        while time.monotonic() < done:
            time.sleep(0.05)
    finally:
        stream.stop()
        stream.close()
    if not chunks:
        return np.zeros(0, dtype=np.float32)
    return np.concatenate(chunks).astype(np.float32) / 32768.0


def main(argv: Optional[list[str]] = None) -> int:
    parser = argparse.ArgumentParser(description="Enroll your voiceprint for Leko speaker isolation.")
    parser.add_argument("--seconds", type=float, default=3.5, help="Recording duration in seconds (default: 3.5)")
    parser.add_argument("--clear", action="store_true", help="Remove saved voiceprint (uses nearest-speaker Focus)")
    parser.add_argument("--out", type=Path, default=VOICEPRINT_PATH, help="Output voiceprint JSON path")
    args = parser.parse_args(argv)

    if args.clear:
        if args.out.exists():
            args.out.unlink()
            print(f"Cleared voiceprint at {args.out}")
        else:
            print("No saved voiceprint found.")
        return 0

    LEKO_SUPPORT_DIR.mkdir(parents=True, exist_ok=True)
    print(f"Speak naturally for {args.seconds:.1f}s to enroll your voiceprint...", flush=True)
    audio = record_clip(args.seconds)
    path = enroll_from_pcm(audio, args.out)
    print(f"Saved voiceprint to {path}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
