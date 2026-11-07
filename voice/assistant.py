"""Leko Voice Assistant: Full interactive loop combining Hands-Free STT, Zero-Model Turn-Taking,
LLM Tool Calling, CLI execution, Maeve TTS, Single-Instance Lock, Serialized Turn Execution,
and Minimalist Circle UI IPC.
"""
from __future__ import annotations

import atexit
import argparse
import os
import queue
import signal
import subprocess
import sys
import threading
import time
from pathlib import Path
from typing import Optional, Dict, Any, Callable

# Ensure we use talk-venv if system python lacks ptts or mlx_audio
TALK_VENV_PYTHON = Path("/Users/calibro/.t3/scratch/phone-pilot/talk-venv/bin/python3")
try:
    import ptts
    import mlx_audio
    import sounddevice as sd
    import onnxruntime
except ImportError:
    if TALK_VENV_PYTHON.exists() and sys.executable != str(TALK_VENV_PYTHON):
        os.execv(str(TALK_VENV_PYTHON), [str(TALK_VENV_PYTHON)] + sys.argv)

import numpy as np
import mlx.core as mx

# Ensure voice package is importable
HERE = Path(__file__).resolve().parent
REPO_ROOT = HERE.parent
sys.path.insert(0, str(HERE))

from llm_router import parse_user_intent_safe, tool_to_cli_args, ensure_server
from ui_bridge import UIPipelineBridge
from turn_detector import TurnDetector, TurnBufferMerger, looks_unfinished

LEKO_BIN = REPO_ROOT / "target/release/leko"
PHONON_25_MODEL = Path("/Users/calibro/.t3/scratch/2026-11-05-can-you-review-this-project-4f6400cb/phonon-2.5-en-mlx")
VOICE_NAME = "Maeve"
DEFAULT_LOCK_FILE = Path("/tmp/leko_assistant.pid")

_tts_engine = None
_stt_model = None
_is_speaking = False
_is_muted = False
_playback_active = False
_playback_decay_until = 0.0

class SerializedTurnWorker:
    """Serializes turn execution on a single worker thread and supersedes stale turns."""
    def __init__(self):
        self.q = queue.Queue()
        self.seq = 0
        self.current_seq = 0
        self.alive = True
        self.lock = threading.Lock()
        self.thread = threading.Thread(target=self._run, daemon=True, name="serialized-turn-worker")
        self.thread.start()

    def submit(self, fn: Callable[[int], None]) -> int:
        with self.lock:
            self.seq += 1
            seq = self.seq
            self.q.put((seq, fn))
            return seq

    def is_superseded(self, seq: int) -> bool:
        with self.lock:
            return seq < self.seq

    def cancel_current(self):
        with self.lock:
            self.seq += 1

    def stop(self):
        self.alive = False
        self.q.put(None)

    def _run(self):
        while self.alive:
            item = self.q.get()
            if item is None or not self.alive:
                break
            seq, fn = item
            with self.lock:
                self.current_seq = seq
            try:
                if not self.is_superseded(seq):
                    fn(seq)
            except Exception as e:
                print(f"[Turn Worker Error]: {e}", file=sys.stderr)
            finally:
                self.q.task_done()

def set_playback_state(is_playing: bool, post_mute_s: float = 0.7) -> None:
    """Manages audio playback state and 0.7s decay mute to prevent room echo into mic."""
    global _playback_active, _playback_decay_until, _is_speaking
    _playback_active = is_playing
    _is_speaking = is_playing
    if not is_playing:
        _playback_decay_until = time.time() + post_mute_s
    else:
        _playback_decay_until = 0.0

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
    set_playback_state(False, post_mute_s=0.2)

def speak(text: str):
    """Speaks text using Maeve via sounddevice with interrupt support and room echo protection."""
    print(f"\n[Maeve speaking]: \"{text}\"", flush=True)
    try:
        tts = get_tts()
        pcm = tts.synth(text, voice=VOICE_NAME)
        set_playback_state(True)
        sd.play(pcm, samplerate=tts.sample_rate)
        sd.wait()
    except Exception as e:
        print(f"[Maeve audio error]: {e}", file=sys.stderr, flush=True)
    finally:
        set_playback_state(False, post_mute_s=0.7)

def get_stt():
    global _stt_model
    if _stt_model is None:
        from mlx_audio.stt.utils import load_model
        _stt_model = load_model(str(PHONON_25_MODEL))
    return _stt_model

def transcribe_audio(audio_f32: np.ndarray) -> str:
    """Transcribes audio samples using local Phonon MLX model."""
    if len(audio_f32) < 3200:  # < 0.2s
        return ""
    stt = get_stt()
    res = stt.generate(mx.array(audio_f32))
    return getattr(res, "text", str(res)).strip()

def process_query(query: str, settings_zip: str, auto_confirm: bool = False, bridge: Optional[UIPipelineBridge] = None) -> str:
    """Processes a user command through LFM3 safely, executes dry run on CLI, and updates UI bridge."""
    print(f"\n[User query]: \"{query}\"")
    if bridge:
        bridge.emit_thinking(query)

    # 1. Parse intent safely through local LFM3
    intent = parse_user_intent_safe(query)

    if intent["action"] == "reply":
        reply = intent.get("content", "I am ready to help manage your blocks and sessions.")
        if bridge:
            bridge.emit_speaking(reply)
        speak(reply)
        if bridge:
            bridge.emit_idle()
        return reply

    tool = intent.get("tool")
    args = intent.get("arguments", {})
    print(f"[LFM3 Tool Call]: {tool}({args})")

    # 2. Map to CLI arguments (dry-run first)
    cli_args = tool_to_cli_args(settings_zip, tool, args, write=False)

    cmd = [str(LEKO_BIN)] + cli_args
    proc = subprocess.run(cmd, capture_output=True, text=True)

    stdout = proc.stdout.strip()
    stderr = proc.stderr.strip()

    if proc.returncode != 0:
        msg = f"Sorry, that request could not be prepared: {stderr or stdout}"
        if bridge:
            bridge.emit_speaking(msg)
        speak(msg)
        if bridge:
            bridge.emit_idle()
        return msg

    print(f"[CLI Output]:\n{stdout}")

    # 3. Formulate concise spoken summary for Maeve & draft metadata for UI card
    if tool == "start_session":
        summary = f"Starting a focus session of {args.get('length')}."
        draft_name = "Focus Session"
        draft_kind = "Session"
        draft_desc = f"Unskippable session for {args.get('length')}."
    elif tool == "add_block":
        targets = []
        if args.get("apps"):
            targets.extend(args["apps"])
        if args.get("sites"):
            targets.extend(args["sites"])
        target_str = ", ".join(targets) if targets else "selected targets"
        summary = f"Drafted a block for {target_str}."
        draft_name = "Block Rule"
        draft_kind = "Block"
        draft_desc = f"Blocks {target_str}."
    elif tool == "switch_off":
        summary = f"Prepared to turn off block {args.get('id')}."
        draft_name = f"Turn Off Block {args.get('id')}"
        draft_kind = "Switch Off"
        draft_desc = f"Turns off rule {args.get('id')}."
    elif tool == "check_status":
        summary = f"Status checked: {stdout.split('.')[0]}."
        draft_name = "Status Report"
        draft_kind = "Status"
        draft_desc = stdout.split('.')[0]
    else:
        summary = "Action prepared."
        draft_name = "Action"
        draft_kind = "Command"
        draft_desc = "Prepared action."

    if bridge:
        bridge.emit_draft(draft_name, draft_kind, draft_desc, raw_intent=intent)
        bridge.emit_speaking(summary)

    speak(summary)

    # 4. If write is applicable and confirmed
    if auto_confirm and tool in ("add_block", "switch_off"):
        write_args = tool_to_cli_args(settings_zip, tool, args, write=True)
        wproc = subprocess.run([str(LEKO_BIN)] + write_args, capture_output=True, text=True)
        if wproc.returncode == 0:
            if bridge:
                bridge.emit_confirmed()
            speak("Changes have been locked and saved.")
        else:
            if bridge:
                bridge.emit_idle()
            speak(f"Could not save changes: {wproc.stderr.strip()}")
    elif not auto_confirm and tool in ("add_block", "switch_off"):
        pass
    else:
        if bridge:
            bridge.emit_idle()

    return summary

class HandsfreeTurnManager:
    def __init__(self, on_turn_complete, on_speech_onset=None, sample_rate: int = 16000, chunk_size: int = 2560):
        self.on_turn_complete = on_turn_complete
        self.sample_rate = sample_rate
        self.chunk_size = chunk_size
        self.detector = TurnDetector(on_speech_onset=on_speech_onset)
        self.merger = TurnBufferMerger(grace_period_s=0.9)
        self.stream = None
        self.is_running = False
        self._raw_buf = bytearray()
        self._lock = threading.Lock()

    def start(self):
        with self._lock:
            self.is_running = True
            self.detector.reset()
            self.merger.clear()
            self._raw_buf.clear()

            def _audio_callback(indata, frames, time_info, status):
                global _is_muted
                if not self.is_running or _is_muted or is_audio_muted_for_echo():
                    return

                self._raw_buf.extend(bytes(indata))
                bytes_per_chunk = self.chunk_size * 2  # 16-bit
                while len(self._raw_buf) >= bytes_per_chunk:
                    chunk_bytes = bytes(self._raw_buf[:bytes_per_chunk])
                    del self._raw_buf[:bytes_per_chunk]
                    chunk = np.frombuffer(chunk_bytes, dtype=np.int16)
                    ended, audio = self.detector.feed(chunk)
                    if ended and len(audio) > 0:
                        full_audio = self.merger.merge_continuation(audio)
                        self.on_turn_complete(full_audio, self.merger)

            self.stream = sd.RawInputStream(
                samplerate=self.sample_rate,
                channels=1,
                dtype="int16",
                blocksize=1024,
                callback=_audio_callback
            )
            self.stream.start()

    def stop(self):
        with self._lock:
            self.is_running = False
            self.merger.clear()
            if self.stream:
                try:
                    self.stream.stop()
                    self.stream.close()
                except Exception:
                    pass
                self.stream = None

def main():
    global _is_muted
    acquire_single_instance_lock()

    parser = argparse.ArgumentParser(description="Leko Voice Assistant")
    parser.add_argument("--settings", default=str(REPO_ROOT / "settings.zip"), help="Path to settings.zip")
    parser.add_argument("--query", type=str, help="Single query to process")
    parser.add_argument("--confirm", action="store_true", help="Auto confirm and write changes")
    parser.add_argument("--ui", action="store_true", help="Launch native WebKit Minimalist UI window and connect IPC")
    args = parser.parse_args()

    bridge = UIPipelineBridge()
    worker = SerializedTurnWorker()
    pending_draft: Dict[str, Any] = {}

    def on_handsfree_speech(audio_f32: np.ndarray, merger: TurnBufferMerger):
        def _execute_turn(seq: int):
            if worker.is_superseded(seq):
                return

            if bridge:
                bridge.emit_state("thinking", "Transcribing...")

            try:
                text = transcribe_audio(audio_f32)
                print(f"\n[Transcribed]: \"{text}\"")

                if worker.is_superseded(seq):
                    return

                if not text:
                    return

                # Dangling guard: if sentence ends on an unfinished grammatical clause, give 1.0s grace period
                if looks_unfinished(text):
                    print(f"[Turn Guard]: Sentence ends on incomplete clause ('{text.split()[-1]}'). Holding for continuation...")
                    merger.push_turn(audio_f32)
                    if bridge:
                        bridge.emit_state("listening", "Listening...")
                    time.sleep(1.0)
                    if worker.is_superseded(seq):
                        return
                    if merger.is_pending_continuation():
                        merger.clear()
                    else:
                        return

                if worker.is_superseded(seq):
                    return

                process_query(text, args.settings, auto_confirm=False, bridge=bridge)
            except Exception as e:
                print(f"[Assistant Error]: {e}", file=sys.stderr)
            finally:
                if bridge and not worker.is_superseded(seq) and not _is_speaking:
                    bridge.emit_idle()

        worker.submit(_execute_turn)

    def on_speech_onset():
        # As soon as user opens mouth to speak: immediately interrupt any prior audio or generation
        stop_playback()
        worker.cancel_current()
        if bridge and not is_audio_muted_for_echo():
            bridge.emit_state("listening", "Listening...")

    turn_manager = HandsfreeTurnManager(
        on_turn_complete=on_handsfree_speech,
        on_speech_onset=on_speech_onset
    )

    def on_ui_action(action_data: Dict[str, Any]):
        global _is_muted
        nonlocal pending_draft
        act = action_data.get("action")
        if act == "listen_start" or act == "listen":
            _is_muted = False
            bridge.emit_state("listening", "Listening...")
        elif act == "stop":
            stop_playback()
            worker.cancel_current()
            turn_manager.detector.reset()
            turn_manager.merger.clear()
            bridge.emit_idle()
            speak("Stopped.")
        elif act == "mute":
            _is_muted = action_data.get("muted", True)
            worker.cancel_current()
            turn_manager.detector.reset()
            turn_manager.merger.clear()
            status_text = "Muted" if _is_muted else "Ready"
            bridge.emit_state("idle", status_text)
        elif act == "confirm":
            draft = action_data.get("draft") or pending_draft
            intent = draft.get("intent", {})
            tool = intent.get("tool")
            tool_args = intent.get("arguments", {})
            if tool and tool in ("add_block", "switch_off"):
                write_args = tool_to_cli_args(args.settings, tool, tool_args, write=True)
                wproc = subprocess.run([str(LEKO_BIN)] + write_args, capture_output=True, text=True)
                if wproc.returncode == 0:
                    bridge.emit_confirmed()
                    speak("Changes have been locked and saved.")
                else:
                    bridge.emit_idle()
                    speak(f"Could not save changes: {wproc.stderr.strip()}")
        elif act == "cancel":
            bridge.emit_idle()
            speak("Action dismissed.")
        elif act == "query":
            q = action_data.get("text", "")
            if q:
                worker.submit(lambda seq: process_query(q, args.settings, auto_confirm=False, bridge=bridge))

    if args.ui:
        print("Launching native Minimalist UI window...")
        bridge.start_ui_process(on_action=on_ui_action)
        time.sleep(0.5)
        bridge.emit_idle()

    # Start hands-free continuous listening loop
    turn_manager.start()

    # Ensure LFM3 server is alive
    ensure_server()

    if args.query:
        process_query(args.query, args.settings, auto_confirm=args.confirm, bridge=bridge)
    else:
        print("\nLeko Voice Assistant ready with single-instance lock and serialized turn control.")
        print("Speak naturally into your mic (or type below, 'exit' to quit):")
        while True:
            try:
                line = input("\n> ").strip()
                if not line or line.lower() in ("exit", "quit"):
                    break
                process_query(line, args.settings, auto_confirm=args.confirm, bridge=bridge)
            except (KeyboardInterrupt, EOFError):
                break

    turn_manager.stop()
    worker.stop()
    if args.ui:
        bridge.close()

if __name__ == "__main__":
    main()
