"""Leko Voice Assistant: Full interactive loop combining STT, LLM Tool Calling, CLI execution, and Maeve TTS.
"""
from __future__ import annotations

import argparse
import os
import subprocess
import sys
from pathlib import Path
from typing import Optional

# Ensure voice package is importable
HERE = Path(__file__).resolve().parent
REPO_ROOT = HERE.parent
sys.path.insert(0, str(HERE))

from llm_router import parse_user_intent, tool_to_cli_args, ensure_server

LEKO_BIN = REPO_ROOT / "target/release/leko"
PHONON_25_MODEL = Path("/Users/calibro/.t3/scratch/2026-11-05-can-you-review-this-project-4f6400cb/phonon-2.5-en-mlx")
VOICE_NAME = "Maeve"

_tts_engine = None
_stt_model = None

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
    """Speaks text using Maeve via sounddevice or saves wav."""
    print(f"\n[Maeve speaking]: \"{text}\"", flush=True)
    tts = get_tts()
    try:
        import sounddevice as sd
        pcm = tts.synth(text, voice=VOICE_NAME)
        sd.play(pcm, samplerate=tts.sample_rate)
        sd.wait()
    except Exception as e:
        # Fallback if audio device not attached in headless environment
        pass

def get_stt():
    global _stt_model
    if _stt_model is None:
        from mlx_audio.stt.utils import load_model
        _stt_model = load_model(str(PHONON_25_MODEL))
    return _stt_model

def process_query(query: str, settings_zip: str, auto_confirm: bool = False) -> str:
    """Processes a user command through LFM3, executes dry run on CLI, and speaks confirmation."""
    print(f"\n[User query]: \"{query}\"")
    
    # 1. Parse intent through local LFM3
    intent = parse_user_intent(query)
    
    if intent["action"] == "reply":
        reply = intent.get("content", "I am ready to help manage your blocks and sessions.")
        speak(reply)
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
        speak(msg)
        return msg

    print(f"[CLI Output]:\n{stdout}")

    # 3. Formulate concise spoken summary for Maeve
    if tool == "start_session":
        summary = f"Starting a focus session of {args.get('length')}."
    elif tool == "add_block":
        targets = []
        if args.get("apps"):
            targets.extend(args["apps"])
        if args.get("sites"):
            targets.extend(args["sites"])
        target_str = ", ".join(targets) if targets else "selected targets"
        summary = f"Drafted a block for {target_str}."
    elif tool == "switch_off":
        summary = f"Prepared to turn off block {args.get('id')}."
    elif tool == "check_status":
        summary = f"Status checked: {stdout.split('.')[0]}."
    else:
        summary = "Action prepared."

    speak(summary)

    # 4. If write is applicable and confirmed
    if auto_confirm and tool in ("add_block", "switch_off"):
        write_args = tool_to_cli_args(settings_zip, tool, args, write=True)
        wproc = subprocess.run([str(LEKO_BIN)] + write_args, capture_output=True, text=True)
        if wproc.returncode == 0:
            speak("Changes have been locked and saved.")
        else:
            speak(f"Could not save changes: {wproc.stderr.strip()}")

    return summary

def main():
    parser = argparse.ArgumentParser(description="Leko Voice Assistant")
    parser.add_argument("--settings", default=str(REPO_ROOT / "settings.zip"), help="Path to settings.zip")
    parser.add_argument("--query", type=str, help="Single query to process")
    parser.add_argument("--confirm", action="store_true", help="Auto confirm and write changes")
    args = parser.parse_args()

    # Ensure LFM3 server is alive
    ensure_server()

    if args.query:
        process_query(args.query, args.settings, auto_confirm=args.confirm)
    else:
        print("Leko Voice Assistant ready. Type your command (or 'exit' to quit):")
        while True:
            try:
                line = input("\n> ").strip()
                if not line or line.lower() in ("exit", "quit"):
                    break
                process_query(line, args.settings, auto_confirm=args.confirm)
            except (KeyboardInterrupt, EOFError):
                break

if __name__ == "__main__":
    main()
