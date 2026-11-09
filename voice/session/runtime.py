"""Runs the hands-free assistant, UI bridge, and serialized query worker."""

import argparse
import json
import re
import time
import threading
import queue
from typing import Any, Dict

import numpy as np
import assistant as voice
from ui_bridge import UIPipelineBridge
from session.conversation import Conversation, confirmation
from session.query_flow import clear_pending

IGNORE_ISOLATED_PHRASES = {
    "yeah", "oh yeah", "yeah yeah", "uh-huh", "uh huh", "mhm", "mm-hmm",
    "yep", "yup", "oh", "ah", "ha", "huh", "mm", "umm", "um", "hmm"
}

def is_ignorable_filler(text: str) -> bool:
    clean = re.sub(r"[^\w\s]", "", text.lower()).strip()
    return clean in IGNORE_ISOLATED_PHRASES


def main():
    parser = argparse.ArgumentParser(description="Leko Voice Assistant")
    parser.add_argument("--settings", default=str(voice.REPO_ROOT / "settings.zip"), help="Path to settings.zip")
    parser.add_argument("--query", type=str, help="Single query to process")
    parser.add_argument("--confirm", action="store_true", help="Auto confirm and write changes")
    parser.add_argument("--ui", action="store_true", help="Launch native WebKit Minimalist UI window and connect IPC")
    parser.add_argument("--speech-detector", choices=("silero", "loudness", "webrtc", "rnnoise", "ten"), default="silero",
                        help="Speech start detection (default: silero)")
    args = parser.parse_args()
    selected_detector = False if args.speech_detector == "loudness" else None
    if args.speech_detector == "webrtc":
        try:
            from audio_io.webrtc_vad import WebRTCVAD
            selected_detector = WebRTCVAD()
            if selected_detector._failed:
                raise RuntimeError("WebRTC VAD failed to initialize")
        except Exception as exc:
            parser.exit(2, f"WebRTC VAD unavailable: {exc}\n")
    elif args.speech_detector == "rnnoise":
        try:
            from audio_io.rnnoise_vad import RNNoiseVAD
            selected_detector = RNNoiseVAD()
        except Exception as exc:
            parser.exit(2, f"RNNoise unavailable: {exc}\n")
    elif args.speech_detector == "ten":
        try:
            from audio_io.ten_vad import TENVAD
            selected_detector = TENVAD()
        except Exception as exc:
            parser.exit(2, f"TEN VAD unavailable: {exc}\n")
    voice.acquire_single_instance_lock()

    bridge = UIPipelineBridge()
    worker = voice.SerializedTurnWorker()
    conversation = Conversation()

    shutdown = threading.Event()

    def submit_response(fn, token=None):
        with turn_manager.response_lock:
            token = turn_manager.begin_response() if token is None else token
            if token is None:
                return False

            def execute(seq):
                turn_manager.wait_paused()
                if not worker.is_superseded(seq) and not shutdown.is_set():
                    fn(seq)

            def finished():
                with turn_manager.response_lock:
                    turn_manager.finish_response(token)
                    if not shutdown.is_set():
                        if voice._is_muted:
                            bridge.emit_state("idle", "Muted")
                        else:
                            bridge.emit_state("listening", "Listening...")
            try:
                worker.submit(execute, on_done=finished)
            except Exception:
                finished()
                raise
            return True

    def on_handsfree_speech(audio_f32: np.ndarray, token):
        def execute_turn(seq):
            bridge.emit_state("thinking", "Transcribing...")
            try:
                diagnostics = turn_manager.take_turn_diagnostics(audio_f32)
                print("[Audio turn] " + json.dumps(diagnostics, default=str, sort_keys=True), flush=True)
            except Exception:
                pass
            stt_started = time.perf_counter()
            stt_result = "error"
            try:
                text = voice.transcribe_audio(audio_f32)
                stt_result = "text" if text else "empty"
            finally:
                print(f"[STT] elapsed={time.perf_counter() - stt_started:.2f}s result={stt_result}", flush=True)
            if worker.is_superseded(seq):
                return
            print(f"\n[Transcribed]: {json.dumps(text or '', ensure_ascii=False)}", flush=True)
            if not text:
                return
            if confirmation(text) is None and is_ignorable_filler(text):
                print("[Turn Guard]: Dropped an isolated filler transcript.", flush=True)
                return
            voice.process_query(text, args.settings, bridge=bridge, conversation=conversation,
                                is_cancelled=lambda: worker.is_superseded(seq))
        submit_response(execute_turn, token)

    def on_speech_onset():
        if not turn_manager.busy and not voice._is_muted:
            bridge.emit_state("listening", "Listening...")

    def on_mic_armed():
        if not voice._is_muted:
            bridge.emit_state("listening", "Listening...")

    turn_manager = voice.HandsfreeTurnManager(
        on_turn_complete=on_handsfree_speech,
        on_speech_onset=on_speech_onset,
        speech_detector=selected_detector,
        on_armed=on_mic_armed,
    )
    selected = args.speech_detector if turn_manager.detector.speech_detector is not None else "loudness"
    print(f"Speech-start detector: {selected}", flush=True)
    print(f"Echo cancellation: {'available' if voice._echo_audio.available else 'unavailable'}", flush=True)

    def cancel_response():
        with turn_manager.response_lock:
            worker.cancel_current()
            turn_manager.set_muted(voice._is_muted)
            voice.stop_playback()
            clear_pending(conversation, bridge)

    def stop_runtime():
        shutdown.set()
        turn_manager.stop()
        cancel_response()
        worker.stop()

    def submit_query(query, confirmation_id=None):
        return submit_response(lambda seq: voice.process_query(
            query, args.settings, auto_confirm=args.confirm, bridge=bridge,
            conversation=conversation, confirmation_id=confirmation_id,
            is_cancelled=lambda: worker.is_superseded(seq)))

    def on_ui_action(action_data: Dict[str, Any]):
        act = action_data.get("action")
        if act == "window_closed":
            stop_runtime()
            return
        if shutdown.is_set():
            return
        if act in ("listen_start", "listen"):
            voice._is_muted = False
            turn_manager.set_muted(False)
            if not turn_manager.busy:
                bridge.emit_state("listening", "Listening...")
        elif act == "stop":
            cancel_response()
        elif act == "mute":
            voice._is_muted = action_data.get("muted", True)
            turn_manager.set_muted(voice._is_muted)
            if voice._is_muted:
                cancel_response()
            bridge.emit_state("idle", "Muted" if voice._is_muted else "Ready")
        elif act in ("confirm", "cancel"):
            action_id = (action_data.get("draft") or {}).get("id")
            with conversation.lock:
                if not action_id or not conversation.pending or conversation.pending["id"] != action_id:
                    return
            if act == "cancel":
                # Cancel is allowed during an active response, without overlapping speech.
                cancel_response()
            else:
                submit_query("yes", action_id)
        elif act == "query":
            query = action_data.get("text", "")
            if query:
                submit_query(query)

    try:
        if args.ui:
            print("Launching native Minimalist UI window...")
            bridge.start_ui_process(on_action=on_ui_action)
            time.sleep(0.5)
            bridge.emit_state("loading", "Loading model...")
        turn_manager.set_muted(voice._is_muted)
        turn_manager.start()

        # MLX streams belong to their creating thread. Load and use STT on the
        # same serialized worker to avoid handing its GPU stream across threads.
        def _warm_stt(seq):
            try:
                voice._stt_lifecycle.warm_cache()
                voice.get_stt()
                print("[STT] Model loaded and ready.", flush=True)
            except Exception as e:
                print(f"[STT] Warm-up failed: {e}", flush=True)
            if args.ui and not turn_manager.busy and not shutdown.is_set():
                bridge.emit_state("listening", "Listening...")

        worker.submit(_warm_stt)
        voice.ensure_server()
        if args.query:
            submit_query(args.query)
            worker.q.join()
        else:
            print("\nLeko Voice Assistant ready with serialized turn control.")
            print("Speak naturally into your mic (or type below, 'exit' to quit):")
            lines = queue.Queue()

            def read_console():
                while not shutdown.is_set():
                    try:
                        line = input("\n> ").strip()
                    except (EOFError, OSError):
                        # No usable console (background launch): keep the
                        # assistant and UI running until shutdown.
                        shutdown.wait()
                        return
                    except KeyboardInterrupt:
                        line = "exit"
                    lines.put(line)
                    if not line or line.lower() in ("exit", "quit"):
                        return

            threading.Thread(target=read_console, daemon=True, name="console-input").start()
            while not shutdown.is_set():
                try:
                    line = lines.get(timeout=0.1)
                except queue.Empty:
                    continue
                if not line or line.lower() in ("exit", "quit"):
                    break
                submit_query(line)
    except KeyboardInterrupt:
        pass
    finally:
        stop_runtime()
        if args.ui:
            bridge.close()
