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


def _parse_args():
    parser = argparse.ArgumentParser(description="Leko Voice Assistant")
    parser.add_argument("--settings", default=str(voice.REPO_ROOT / "settings.zip"), help="Path to settings.zip")
    parser.add_argument("--query", type=str, help="Single query to process")
    parser.add_argument("--confirm", action="store_true", help="Auto confirm and write changes")
    parser.add_argument("--ui", action="store_true", help="Launch native WebKit Minimalist UI window and connect IPC")
    parser.add_argument("--speech-detector", choices=("silero", "loudness", "webrtc", "rnnoise", "ten"), default="silero",
                        help="Speech start detection (default: silero)")
    return parser, parser.parse_args()


def _select_speech_detector(args, parser):
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
    return selected_detector


class RuntimeController:
    def __init__(self, args, selected_detector):
        self.args = args
        self.bridge = UIPipelineBridge()
        self.worker = voice.SerializedTurnWorker()
        self.conversation = Conversation()
        self.shutdown = threading.Event()
        self.turn_manager = voice.HandsfreeTurnManager(
            on_turn_complete=self.on_handsfree_speech,
            on_speech_onset=self.on_speech_onset,
            speech_detector=selected_detector,
            on_armed=self.on_mic_armed,
        )
        selected = args.speech_detector if self.turn_manager.detector.speech_detector is not None else "loudness"
        print(f"Speech-start detector: {selected}", flush=True)
        print(f"Echo cancellation: {'available' if voice._echo_audio.available else 'unavailable'}", flush=True)

    def submit_response(self, fn, token=None):
        with self.turn_manager.response_lock:
            token = self.turn_manager.begin_response() if token is None else token
            if token is None:
                return False

            def execute(seq):
                self.turn_manager.wait_paused()
                if not self.worker.is_superseded(seq) and not self.shutdown.is_set():
                    fn(seq)

            def finished():
                with self.turn_manager.response_lock:
                    self.turn_manager.finish_response(token)
                    if not self.shutdown.is_set():
                        if voice._is_muted:
                            self.bridge.emit_state("idle", "Muted")
                        else:
                            self.bridge.emit_state("listening", "Listening...")
            try:
                self.worker.submit(execute, on_done=finished)
            except Exception:
                finished()
                raise
            return True

    def on_handsfree_speech(self, audio_f32: np.ndarray, token):
        def execute_turn(seq):
            self.bridge.emit_state("thinking", "Transcribing...")
            try:
                diagnostics = self.turn_manager.take_turn_diagnostics(audio_f32)
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
            if self.worker.is_superseded(seq):
                return
            print(f"\n[Transcribed]: {json.dumps(text or '', ensure_ascii=False)}", flush=True)
            if not text:
                return
            if confirmation(text) is None and is_ignorable_filler(text):
                print("[Turn Guard]: Dropped an isolated filler transcript.", flush=True)
                return
            voice.process_query(text, self.args.settings, bridge=self.bridge, conversation=self.conversation,
                                is_cancelled=lambda: self.worker.is_superseded(seq))
        self.submit_response(execute_turn, token)

    def on_speech_onset(self):
        if not self.turn_manager.busy and not voice._is_muted:
            self.bridge.emit_state("listening", "Listening...")

    def on_mic_armed(self):
        if not voice._is_muted:
            self.bridge.emit_state("listening", "Listening...")

    def cancel_response(self):
        with self.turn_manager.response_lock:
            self.worker.cancel_current()
            self.turn_manager.set_muted(voice._is_muted)
            voice.stop_playback()
            clear_pending(self.conversation, self.bridge)

    def stop_runtime(self):
        self.shutdown.set()
        self.turn_manager.stop()
        self.cancel_response()
        self.worker.stop()

    def submit_query(self, query, confirmation_id=None):
        return self.submit_response(lambda seq: voice.process_query(
            query, self.args.settings, auto_confirm=self.args.confirm, bridge=self.bridge,
            conversation=self.conversation, confirmation_id=confirmation_id,
            is_cancelled=lambda: self.worker.is_superseded(seq)))

    def on_ui_action(self, action_data: Dict[str, Any]):
        act = action_data.get("action")
        if act == "window_closed":
            self.stop_runtime()
            return
        if self.shutdown.is_set():
            return
        if act in ("listen_start", "listen"):
            voice._is_muted = False
            self.turn_manager.set_muted(False)
            if not self.turn_manager.busy:
                self.bridge.emit_state("listening", "Listening...")
        elif act == "stop":
            self.cancel_response()
        elif act == "mute":
            voice._is_muted = action_data.get("muted", True)
            self.turn_manager.set_muted(voice._is_muted)
            if voice._is_muted:
                self.cancel_response()
            self.bridge.emit_state("idle", "Muted" if voice._is_muted else "Ready")
        elif act in ("confirm", "cancel"):
            action_id = (action_data.get("draft") or {}).get("id")
            with self.conversation.lock:
                if not action_id or not self.conversation.pending or self.conversation.pending["id"] != action_id:
                    return
            if act == "cancel":
                # Cancel is allowed during an active response, without overlapping speech.
                self.cancel_response()
            else:
                self.submit_query("yes", action_id)
        elif act == "query":
            query = action_data.get("text", "")
            if query:
                self.submit_query(query)

    def _warm_stt(self, seq):
        try:
            voice._stt_lifecycle.warm_cache()
            voice.get_stt()
            print("[STT] Model loaded and ready.", flush=True)
        except Exception as exc:
            print(f"[STT] Warm-up failed: {exc}", flush=True)
        if self.args.ui and not self.turn_manager.busy and not self.shutdown.is_set():
            self.bridge.emit_state("listening", "Listening...")

    def _run_console(self):
        print("\nLeko Voice Assistant ready with serialized turn control.")
        print("Speak naturally into your mic (or type below, 'exit' to quit):")
        lines = queue.Queue()

        def read_console():
            while not self.shutdown.is_set():
                try:
                    line = input("\n> ").strip()
                except (EOFError, OSError):
                    # Background launches keep the UI alive without a console.
                    self.shutdown.wait()
                    return
                except KeyboardInterrupt:
                    line = "exit"
                lines.put(line)
                if not line or line.lower() in ("exit", "quit"):
                    return

        threading.Thread(target=read_console, daemon=True, name="console-input").start()
        while not self.shutdown.is_set():
            try:
                line = lines.get(timeout=0.1)
            except queue.Empty:
                continue
            if not line or line.lower() in ("exit", "quit"):
                break
            self.submit_query(line)

    def run(self):
        try:
            if self.args.ui:
                print("Launching native Minimalist UI window...")
                self.bridge.start_ui_process(on_action=self.on_ui_action)
                time.sleep(0.5)
                self.bridge.emit_state("loading", "Loading model...")
            self.turn_manager.set_muted(voice._is_muted)
            self.turn_manager.start()
            # Load MLX STT on the same serialized worker that will use it.
            self.worker.submit(self._warm_stt)
            voice.ensure_server()
            if self.args.query:
                self.submit_query(self.args.query)
                self.worker.q.join()
            else:
                self._run_console()
        except KeyboardInterrupt:
            pass
        finally:
            self.stop_runtime()
            if self.args.ui:
                self.bridge.close()


def main():
    parser, args = _parse_args()
    selected_detector = _select_speech_detector(args, parser)
    voice.acquire_single_instance_lock()
    RuntimeController(args, selected_detector).run()
