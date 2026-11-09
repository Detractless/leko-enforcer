"""Leko Voice Assistant: Full interactive loop combining Hands-Free STT, Zero-Model Turn-Taking,
LLM Tool Calling, CLI execution, Maeve TTS, Single-Instance Lock, Serialized Turn Execution,
and Minimalist Circle UI IPC.
"""
from __future__ import annotations

import atexit
import os
import re
import signal
import subprocess
import sys
import time
from pathlib import Path
from typing import Optional, Callable

# Ensure we use talk-venv if system python lacks dependencies
TALK_VENV_PYTHON = Path("/Users/calibro/.t3/scratch/phone-pilot/talk-venv/bin/python3")
try:
    import ptts
    import sounddevice as sd
    import importlib.util
    if any(importlib.util.find_spec(mod) is None for mod in ("mlx_audio", "onnxruntime")):
        raise ImportError
except ImportError:
    if TALK_VENV_PYTHON.exists() and sys.executable != str(TALK_VENV_PYTHON):
        os.execv(str(TALK_VENV_PYTHON), [str(TALK_VENV_PYTHON)] + sys.argv)

import numpy as np
import mlx.core as mx

# Ensure voice package is importable
HERE = Path(__file__).resolve().parent
REPO_ROOT = HERE.parent
sys.path.insert(0, str(HERE))
sys.modules.setdefault("assistant", sys.modules[__name__])

from verbalizer import verbalize_for_speech
from turn_detector import TurnDetector
from stt_lifecycle import SttLifecycle
from llm_router import parse_user_intent_safe, tool_to_cli_args, ensure_server
from session.turn_worker import SerializedTurnWorker

__all__ = [
    'parse_user_intent_safe',
    'tool_to_cli_args',
    'ensure_server',
    'SerializedTurnWorker',
    'process_query',
    'format_status_summary',
    'speak',
    'stop_playback',
    'acquire_single_instance_lock',
    'is_audio_muted_for_echo',
    'set_playback_state',
    'HandsfreeTurnManager',
    'TurnDetector',
    'verbalize_for_speech',
]
from audio_io.echo_canceller import EchoAudioProcessor, VoiceOutputPlayer

LEKO_BIN = REPO_ROOT / "target/release/leko"
PHONON_25_MODEL = Path("/Users/calibro/.t3/scratch/2026-11-05-can-you-review-this-project-4f6400cb/phonon-2.5-en-mlx")
VOICE_NAME = "Zoey"
DEFAULT_LOCK_FILE = Path("/tmp/leko_assistant.pid")

_tts_engine = None
def _load_stt_model(path):
    from mlx_audio.stt.utils import load_model
    return load_model(str(path))

_stt_lifecycle = SttLifecycle(PHONON_25_MODEL, _load_stt_model, idle_timeout_s=300.0)
_is_speaking = False
_is_muted = False
_playback_active = False
_playback_decay_until = 0.0
_echo_audio = EchoAudioProcessor()
_output_player = VoiceOutputPlayer(_echo_audio)

_active_turn_detector = None

_playback_start_time = 0.0

def set_playback_state(is_playing: bool, post_mute_s: float = 0.25) -> None:
    """Manages audio playback state and echo defense."""
    global _playback_active, _playback_decay_until, _is_speaking, _active_turn_detector, _playback_start_time
    _playback_active = is_playing
    _is_speaking = is_playing
    if is_playing:
        _playback_start_time = time.monotonic()
        _playback_decay_until = 0.0
    else:
        _playback_decay_until = time.time() + post_mute_s
    if _active_turn_detector is not None:
        try:
            _active_turn_detector.set_playback_active(is_playing, post_mute_s=post_mute_s)
        except Exception:
            pass

def set_echo_cancellation_enabled(enabled: bool) -> None:
    """Switches to guarded half-duplex audio if the AEC path fails."""
    if _active_turn_detector is not None:
        _active_turn_detector.set_echo_cancellation_enabled(enabled)

def is_audio_muted_for_echo() -> bool:
    """Returns True if mic must be muted due to active playback or room echo decay."""
    global _playback_active, _playback_decay_until
    return _playback_active or (time.time() < _playback_decay_until)

def acquire_single_instance_lock(lock_file: Optional[Path] = None) -> bool:
    """Ensures only a single assistant & UI window run at any time, terminating previous instances."""
    lock_path = lock_file or DEFAULT_LOCK_FILE
    current_pid = os.getpid()

    if lock_path.exists():
        try:
            old_pid = int(lock_path.read_text().strip())
            if old_pid != current_pid:
                try:
                    os.kill(old_pid, 0)
                    os.kill(old_pid, signal.SIGTERM)
                    time.sleep(0.3)
                    try:
                        os.kill(old_pid, signal.SIGKILL)
                    except ProcessLookupError:
                        pass
                except (ProcessLookupError, PermissionError):
                    pass
        except Exception:
            pass

    try:
        subprocess.run(["pkill", "-f", "leko-ui"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        time.sleep(0.15)
    except Exception:
        pass

    try:
        lock_path.write_text(str(current_pid))
        def _cleanup():
            try:
                if lock_path.exists() and lock_path.read_text().strip() == str(current_pid):
                    lock_path.unlink()
            except Exception:
                pass
        atexit.register(_cleanup)
        return True
    except Exception:
        return False

def get_tts():
    global _tts_engine
    if _tts_engine is None:
        import ptts
        _tts_engine = ptts.TTS(
            "hf://gradium/phonon-31m-en",
            lang="en",
            quant="f32",
            voice_catalog="hf://gradium/phonon-voices"
        )
    return _tts_engine

def stop_playback():
    """Interrupts any active audio playback safely, suppressing AUHAL abort stderr errors."""
    try:
        _output_player.stop()
        old_stderr = os.dup(2)
        devnull = os.open(os.devnull, os.O_WRONLY)
        os.dup2(devnull, 2)
        try:
            sd.stop()
        finally:
            os.dup2(old_stderr, 2)
            os.close(old_stderr)
            os.close(devnull)
    except Exception:
        pass
    set_playback_state(False, post_mute_s=0.0 if _echo_audio.available else 0.25)

def speak(text: str, is_cancelled: Optional[Callable[[], bool]] = None):
    """Speaks text using Maeve via sounddevice with interrupt support and room echo protection."""
    if is_cancelled and is_cancelled():
        return
    spoken_text = verbalize_for_speech(text)
    if not spoken_text:
        return
    try:
        tts = get_tts()
        if is_cancelled and is_cancelled():
            return
        pcm = tts.synth(spoken_text, voice=VOICE_NAME)
        if is_cancelled and is_cancelled():
            return
        duration_s = len(pcm) / tts.sample_rate if hasattr(pcm, "__len__") else 0.0
        print(f"\n[Maeve speaking] (expected duration: {duration_s:.2f}s):\n\"{spoken_text}\"", flush=True)
        set_echo_cancellation_enabled(_echo_audio.available)
        set_playback_state(True)
        if _output_player.play(pcm, tts.sample_rate, is_cancelled):
            return
        set_echo_cancellation_enabled(False)
        play_start_t = time.monotonic()
        was_interrupted = False
        if is_cancelled and is_cancelled():
            return
        sd.play(pcm, samplerate=tts.sample_rate)
        while True:
            if is_cancelled and is_cancelled():
                was_interrupted = True
                stop_playback()
                break
            stream = sd.get_stream()
            if stream is None or not stream.active:
                break
            time.sleep(0.04)
        elapsed_s = time.monotonic() - play_start_t
        if was_interrupted:
            print(f"[Maeve playback interrupted]: stopped at {elapsed_s:.2f}s / {duration_s:.2f}s", flush=True)
        else:
            print(f"[Maeve playback completed]: finished full {elapsed_s:.2f}s / {duration_s:.2f}s", flush=True)
    except Exception as e:
        print(f"[Maeve audio error]: {e}", file=sys.stderr, flush=True)
        stop_playback()
    finally:
        set_playback_state(False, post_mute_s=0.0 if _echo_audio.available else 0.25)

def get_stt():
    return _stt_lifecycle.get_model()

def transcribe_audio(audio_f32: np.ndarray) -> str:
    """Transcribes audio samples using local Phonon MLX model."""
    if len(audio_f32) < 3200:  # < 0.2s
        return ""
    chunk_size = 1600  # 100ms at 16kHz
    num_chunks = len(audio_f32) // chunk_size
    if num_chunks > 0:
        max_chunk_rms = max(
            float(np.sqrt(np.mean(audio_f32[i * chunk_size:(i + 1) * chunk_size] ** 2)))
            for i in range(num_chunks)
        )
        if max_chunk_rms < 0.008:
            return ""
    stt = get_stt()
    res = stt.generate(mx.array(audio_f32))
    return getattr(res, "text", str(res)).strip()

def format_status_summary(raw_stdout: str, target_type: str = "", name: str = "") -> str:
    """Formats raw CLI status output directly into a clean, natural spoken sentence."""
    if not raw_stdout:
        return f"{name} status is unavailable." if name else "Status check completed."

    first_line = raw_stdout.strip().split("\n")[0].strip()
    parts = re.split(r"\.\s+", first_line)
    first_sentence = parts[0].strip()

    if first_sentence.startswith("Site "):
        first_sentence = first_sentence[5:]
    elif first_sentence.startswith("App "):
        first_sentence = first_sentence[4:]
    elif first_sentence.startswith("Device: "):
        first_sentence = first_sentence[8:]

    if ", judged at" in first_sentence:
        first_sentence = first_sentence.split(", judged at")[0].strip()

    return f"{first_sentence}." 

def process_query(*args, **kwargs) -> str:
    """Compatibility export for the separated query flow."""
    from session.query_flow import process_query as run_query
    return run_query(*args, **kwargs)

from audio_io.capture_gate import CaptureGate

class HandsfreeTurnManager(CaptureGate):
    def __init__(self, on_turn_complete, on_speech_onset=None, sample_rate=16000,
                 chunk_size=2560, speech_detector=None, continuation_s=4.0, on_armed=None):
        detector = TurnDetector(on_speech_onset=on_speech_onset, speech_detector=speech_detector, continuation_s=continuation_s)
        super().__init__(sd.RawInputStream, detector, _echo_audio, on_turn_complete,
                         sample_rate, chunk_size, on_armed=on_armed)

    def start(self):
        global _active_turn_detector
        _active_turn_detector = self.detector
        super().start()

    def stop(self):
        global _active_turn_detector
        super().stop()
        if _active_turn_detector is self.detector:
            _active_turn_detector = None


def main():
    """Compatibility entrypoint for the separated assistant runtime."""
    from session.runtime import main as run
    return run()

if __name__ == "__main__":
    main()
