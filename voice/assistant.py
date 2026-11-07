"""Leko Voice Assistant: Full interactive loop combining STT, LLM Tool Calling, CLI execution, Maeve TTS, and Minimalist Circle UI IPC.
"""
from __future__ import annotations

import argparse
import os
import subprocess
import sys
import threading
import time
from pathlib import Path
from typing import Optional, Dict, Any

# Ensure we use talk-venv if system python lacks ptts or mlx_audio
TALK_VENV_PYTHON = Path("/Users/calibro/.t3/scratch/phone-pilot/talk-venv/bin/python3")
try:
    import ptts
    import mlx_audio
    import sounddevice
except ImportError:
    if TALK_VENV_PYTHON.exists() and sys.executable != str(TALK_VENV_PYTHON):
        os.execv(str(TALK_VENV_PYTHON), [str(TALK_VENV_PYTHON)] + sys.argv)

import numpy as np
import mlx.core as mx

# Ensure voice package is importable
HERE = Path(__file__).resolve().parent
REPO_ROOT = HERE.parent
sys.path.insert(0, str(HERE))

from llm_router import parse_user_intent, tool_to_cli_args, ensure_server
from ui_bridge import UIPipelineBridge

LEKO_BIN = REPO_ROOT / "target/release/leko"
PHONON_25_MODEL = Path("/Users/calibro/.t3/scratch/2026-11-05-can-you-review-this-project-4f6400cb/phonon-2.5-en-mlx")
VOICE_NAME = "Maeve"

_tts_engine = None
_stt_model = None

class AudioRecorder:
    def __init__(self, sample_rate: int = 16000):
        self.sample_rate = sample_rate
        self.stream = None
        self.frames = []
        self.is_recording = False
        self._lock = threading.Lock()

    def start(self):
        import sounddevice as sd
        with self._lock:
            self.frames = []
            self.is_recording = True

            def callback(indata, frame_count, time_info, status):
                if self.is_recording:
                    self.frames.append(bytes(indata))

            self.stream = sd.RawInputStream(
                samplerate=self.sample_rate,
                channels=1,
                dtype="int16",
                blocksize=1024,
                callback=callback
            )
            self.stream.start()

    def stop(self) -> np.ndarray:
        with self._lock:
            self.is_recording = False
            if self.stream:
                try:
                    self.stream.stop()
                    self.stream.close()
                except Exception:
                    pass
                self.stream = None
            if not self.frames:
                return np.zeros(0, dtype=np.float32)
            raw = b"".join(self.frames)
            i16 = np.frombuffer(raw, dtype=np.int16)
            return i16.astype(np.float32) / 32768.0

_recorder = AudioRecorder()

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

def speak(text: str):
    """Speaks text using Maeve via sounddevice."""
    print(f"\n[Maeve speaking]: \"{text}\"", flush=True)
    try:
        tts = get_tts()
        import sounddevice as sd
        pcm = tts.synth(text, voice=VOICE_NAME)
        sd.play(pcm, samplerate=tts.sample_rate)
        sd.wait()
    except Exception as e:
        print(f"[Maeve audio error]: {e}", file=sys.stderr, flush=True)

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
    """Processes a user command through LFM3, executes dry run on CLI, and updates UI bridge."""
    print(f"\n[User query]: \"{query}\"")
    if bridge:
        bridge.emit_thinking(query)

    # 1. Parse intent through local LFM3
    intent = parse_user_intent(query)

    if intent["action"] == "reply":
        reply = intent.get("content", "I am ready to help manage your blocks and sessions.")
        if bridge:
            bridge.emit_speaking(reply)
        speak(reply)
        if bridge:
            bridge.emit_idle()
        return reply

    tool = intent["tool"]
    args = intent["arguments"]
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
        # UI confirmation card remains visible waiting for user confirm/cancel action
        pass
    else:
        if bridge:
            bridge.emit_idle()

    return summary

def main():
    parser = argparse.ArgumentParser(description="Leko Voice Assistant")
    parser.add_argument("--settings", default=str(REPO_ROOT / "settings.zip"), help="Path to settings.zip")
    parser.add_argument("--query", type=str, help="Single query to process")
    parser.add_argument("--confirm", action="store_true", help="Auto confirm and write changes")
    parser.add_argument("--ui", action="store_true", help="Launch native WebKit Minimalist UI window and connect IPC")
    args = parser.parse_args()

    bridge = UIPipelineBridge()

    pending_draft: Dict[str, Any] = {}

    def on_ui_action(action_data: Dict[str, Any]):
        nonlocal pending_draft
        act = action_data.get("action")
        if act == "listen_start" or act == "listen":
            print("\n[Microphone]: Recording started... (Speak now, then tap circle to end)")
            _recorder.start()
        elif act == "listen_stop":
            print("\n[Microphone]: Recording stopped. Transcribing...")
            audio = _recorder.stop()
            transcription = transcribe_audio(audio)
            print(f"[Transcription]: \"{transcription}\"")
            if not transcription:
                if bridge:
                    bridge.emit_idle()
                speak("I didn't catch that. Please tap the circle and try again.")
            else:
                process_query(transcription, args.settings, auto_confirm=False, bridge=bridge)
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
                process_query(q, args.settings, auto_confirm=False, bridge=bridge)

    if args.ui:
        print("Launching native Minimalist UI window...")
        bridge.start_ui_process(on_action=on_ui_action)
        time.sleep(0.5)
        bridge.emit_idle()

    # Ensure LFM3 server is alive
    ensure_server()

    if args.query:
        process_query(args.query, args.settings, auto_confirm=args.confirm, bridge=bridge)
    else:
        print("Leko Voice Assistant ready. Tap the circle in the UI or type here (or 'exit' to quit):")
        while True:
            try:
                line = input("\n> ").strip()
                if not line or line.lower() in ("exit", "quit"):
                    break
                process_query(line, args.settings, auto_confirm=args.confirm, bridge=bridge)
            except (KeyboardInterrupt, EOFError):
                break

    if args.ui:
        bridge.close()

if __name__ == "__main__":
    main()
