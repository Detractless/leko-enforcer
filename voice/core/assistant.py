"""Leko Voice Assistant: Hands-free/PTT STT, LLM/deterministic routing, Maeve TTS, and Pill UI IPC."""
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
    pass

import numpy as np
import mlx.core as mx

# Ensure voice package is importable
HERE = Path(__file__).resolve().parent
REPO_ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(HERE))
sys.modules.setdefault("assistant", sys.modules[__name__])

from ..normalization.verbalizer import verbalize_for_speech
from .turn_detector import TurnDetector
from .stream_recovery import open_input_stream, pad_bluetooth_preroll, sync_output_device
from .ui_bridge import compute_word_timings_ms, load_card_timing, start_playback_sampler
try:
    from ..stt.stt_lifecycle import SttLifecycle
except ImportError:
    from stt.stt_lifecycle import SttLifecycle
from ..routing.intent_validation import tool_to_cli_args
from ..routing.canonical_speech import format_status_summary as _canonical_status
from ..routing.llm_request import parse_request_update_safe, parse_user_intent_safe
from ..routing.llm_router import ensure_server
from ..session.turn_worker import SerializedTurnWorker

__all__ = [
    'parse_user_intent_safe', 'parse_request_update_safe', 'tool_to_cli_args', 'ensure_server',
    'SerializedTurnWorker', 'process_query', 'format_status_summary', 'speak', 'stop_playback',
    'acquire_single_instance_lock', 'is_audio_muted_for_echo', 'set_playback_state',
    'HandsfreeTurnManager', 'PushToTalkTurnManager', 'TurnDetector', 'verbalize_for_speech',
]
from .alkove_isolator import IdleSafeEchoAudio, prepare_audio_for_stt
from ..audio_io.echo_canceller import EchoAudioProcessor, VoiceOutputPlayer

LEKO_BIN = Path(os.environ.get("LEKO_CLI_BIN", str(REPO_ROOT / "target/release/leko")))
PHONON_25_MODEL = Path("/Users/calibro/.t3/scratch/2026-11-05-can-you-review-this-project-4f6400cb/phonon-2.5-en-mlx")
VOICE_NAME = "Zoey"
DEFAULT_LOCK_FILE = Path("/tmp/leko_assistant.pid")

_tts_engine = None
def _load_stt_model(path):
    from mlx_audio.stt.utils import load_model
    return load_model(str(path))
_stt_lifecycle = SttLifecycle(PHONON_25_MODEL, _load_stt_model, idle_timeout_s=300.0)
_is_speaking = _is_muted = _playback_active = False
_playback_decay_until = _playback_start_time = 0.0
_echo_audio = IdleSafeEchoAudio(EchoAudioProcessor())
_output_player = VoiceOutputPlayer(_echo_audio)
_active_turn_detector = None
_active_input_stream = None

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
        subprocess.run(["pkill", "-f", "leko-pill"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
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
            "hf://gradium/phonon-31m-en", lang="en", quant="f32",
            voice_catalog="hf://gradium/phonon-voices",
        )
    return _tts_engine

def warm_tts():
    tts = get_tts()
    tts.synth("Ready.", voice=VOICE_NAME)
    try:
        sr = int(getattr(tts, "sample_rate", 24000) or 24000)
        _output_player.play(np.zeros(max(1, sr // 20), dtype=np.float32), sr)
    except Exception:
        pass
    return tts

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

def speak(text: str, is_cancelled: Optional[Callable[[], bool]] = None,
          playback_event_sink: Optional[Callable[[dict], None]] = None,
          on_audio_level: Optional[Callable[[float], None]] = None):
    """Speaks text using Maeve via sounddevice with interrupt support and room echo protection."""
    def emit(event, **details):
        if playback_event_sink:
            try:
                playback_event_sink({"status": event, **details})
            except Exception as exc:
                print(f"[Session log warning] playback event could not be saved: {exc}", file=sys.stderr, flush=True)

    if is_cancelled and is_cancelled():
        emit("skipped", reason="cancelled_before_synthesis")
        return
    spoken_text = verbalize_for_speech(text)
    if not spoken_text:
        emit("skipped", reason="empty_after_verbalization")
        return
    try:
        tts = get_tts()
        if is_cancelled and is_cancelled():
            return
        pcm = tts.synth(spoken_text, voice=VOICE_NAME)
        if is_cancelled and is_cancelled():
            return
        sync_fn = globals().get("sync_output_device")
        if sync_fn:
            sync_fn(sd, can_refresh=lambda: _active_input_stream is None or getattr(_active_input_stream, "closed", True))
        pad_fn = globals().get("pad_bluetooth_preroll")
        if pad_fn:
            pcm = pad_fn(pcm, tts.sample_rate, sd_module=sd)
        duration_s = len(pcm) / tts.sample_rate if hasattr(pcm, "__len__") else 0.0
        word_timings_ms = globals()["compute_word_timings_ms"](pcm, tts.sample_rate, spoken_text) if "compute_word_timings_ms" in globals() else None
        card_cfg = globals()["load_card_timing"]() if "load_card_timing" in globals() else {}
        print(f"\n[Maeve speaking] (expected duration: {duration_s:.2f}s):\n\"{spoken_text}\"", flush=True)
        set_echo_cancellation_enabled(_echo_audio.available)
        set_playback_state(True)
        emit("started", text=spoken_text, voice=VOICE_NAME, sample_rate=tts.sample_rate,
             expected_duration_seconds=duration_s, word_timings_ms=word_timings_ms,
             reveal_mode=card_cfg.get("reveal_mode"))
        sampler = globals().get("start_playback_sampler")
        play_started = time.monotonic()
        stop_meter = sampler(pcm, tts.sample_rate, on_audio_level) if sampler else (lambda: None)
        try:
            if _output_player.play(pcm, tts.sample_rate, is_cancelled):
                emit("completed", elapsed_seconds=round(time.monotonic() - play_started, 3))
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
        finally:
            stop_meter()
        elapsed_s = time.monotonic() - play_start_t
        if was_interrupted:
            print(f"[Maeve playback interrupted]: stopped at {elapsed_s:.2f}s / {duration_s:.2f}s", flush=True)
            emit("interrupted", elapsed_seconds=round(elapsed_s, 3), expected_duration_seconds=duration_s)
        else:
            print(f"[Maeve playback completed]: finished full {elapsed_s:.2f}s / {duration_s:.2f}s", flush=True)
            emit("completed", elapsed_seconds=round(elapsed_s, 3), expected_duration_seconds=duration_s)
    except Exception as e:
        print(f"[Maeve audio error]: {e}", file=sys.stderr, flush=True)
        emit("error", error={"type": type(e).__name__, "message": str(e)})
        stop_playback()
    finally:
        set_playback_state(False, post_mute_s=0.0 if _echo_audio.available else 0.25)

def get_stt():
    return _stt_lifecycle.get_model()

def transcribe_audio(audio_f32: np.ndarray) -> str:
    """Transcribes isolated, normalized audio samples using local Phonon MLX model."""
    prepared = prepare_audio_for_stt(audio_f32)
    if prepared is None:
        return ""
    stt = get_stt()
    res = stt.generate(mx.array(prepared))
    return getattr(res, "text", str(res)).strip()

def format_status_summary(raw_stdout: str, target_type: str = "", name: str = "") -> str:
    """Format native CLI JSON into one exact Title Case spoken status sentence."""
    return _canonical_status(raw_stdout, target_type, name)

def process_query(*args, **kwargs) -> str:
    """Compatibility export for the separated query flow."""
    from ..session.query_flow import process_query as run_query
    return run_query(*args, **kwargs)

from ..audio_io.capture_gate import CaptureGate
from ..audio_io.push_to_talk.push_to_talk import PushToTalkTurnManager as _PushToTalkTurnManager

def _clean_with_meter(super_clean, raw_samples, on_audio_level):
    samples, failed = super_clean(raw_samples)
    if not failed and on_audio_level and samples is not None and len(samples):
        norm = samples.astype(np.float32) / 32768.0
        on_audio_level(float(np.sqrt(np.mean(norm * norm))))
    return samples, failed

def _open_mic_stream(**kw):
    global _active_input_stream
    stream = open_input_stream(sd, can_refresh=lambda: _output_player.stream is None, **kw)
    _active_input_stream = stream
    return stream

class HandsfreeTurnManager(CaptureGate):
    def __init__(self, on_turn_complete, on_speech_onset=None, sample_rate=16000,
                 chunk_size=2560, speech_detector=None, continuation_s=4.0, on_armed=None, on_audio_level=None):
        self._on_audio_level = on_audio_level
        detector = TurnDetector(on_speech_onset=on_speech_onset, speech_detector=speech_detector, continuation_s=continuation_s)
        super().__init__(_open_mic_stream, detector, _echo_audio, on_turn_complete, sample_rate, chunk_size, on_armed=on_armed)

    def _clean_samples(self, raw_samples):
        return _clean_with_meter(super()._clean_samples, raw_samples, self._on_audio_level)

    def start(self):
        global _active_turn_detector
        _active_turn_detector = self.detector
        super().start()

    def stop(self):
        global _active_turn_detector
        super().stop()
        if _active_turn_detector is self.detector:
            _active_turn_detector = None

class PushToTalkTurnManager(_PushToTalkTurnManager):
    def __init__(self, on_turn_complete, on_speech_onset=None, on_capture_error=None,
                 sample_rate=16000, chunk_size=1280, grace_s=2.0, on_audio_level=None):
        self._on_audio_level = on_audio_level
        super().__init__(_open_mic_stream, _echo_audio, on_turn_complete, sample_rate=sample_rate,
                         chunk_size=chunk_size, on_speech_onset=on_speech_onset, on_capture_error=on_capture_error, grace_s=grace_s)

    def _clean_samples(self, raw_samples):
        return _clean_with_meter(super()._clean_samples, raw_samples, self._on_audio_level)

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
    from ..session.runtime import main as run
    return run()

if __name__ == "__main__":
    main()
