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
from session.session_log import SessionLog
from session.query_flow import clear_pending
from settings_setup import SettingsSetupError, prepare_settings

IGNORE_ISOLATED_PHRASES = {
    "yeah", "oh yeah", "yeah yeah", "uh-huh", "uh huh", "mhm", "mm-hmm",
    "yep", "yup", "oh", "ah", "ha", "huh", "mm", "umm", "um", "hmm"
}

def is_ignorable_filler(text: str) -> bool:
    clean = re.sub(r"[^\w\s]", "", text.lower()).strip()
    return clean in IGNORE_ISOLATED_PHRASES


def _parse_args():
    parser = argparse.ArgumentParser(description="Leko Voice Assistant")
    parser.add_argument("--settings", help="Optional settings override; normally managed automatically")
    parser.add_argument("--query", type=str, help="Single query to process")
    parser.add_argument("--confirm", action="store_true", help="Auto confirm and write changes")
    parser.add_argument("--ui", action="store_true", help="Launch native WebKit Minimalist UI window and connect IPC")
    parser.add_argument("--speech-detector", choices=("silero", "loudness", "webrtc", "rnnoise", "ten"), default="silero",
                        help="Speech start detection (default: silero)")
    return parser, parser.parse_args()


def _select_speech_detector(args, parser):
    if args.ui:
        return None
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
        self.session_log = SessionLog()
        self.conversation.session_log = self.session_log
        self.shutdown = threading.Event()
        self._active_response_token = None
        self._capture_turn_id = None
        if args.ui:
            self.turn_manager = voice.PushToTalkTurnManager(
                on_turn_complete=self.on_handsfree_speech,
                on_capture_error=self.on_capture_error,
            )
            selected = "push-to-talk"
        else:
            self.turn_manager = voice.HandsfreeTurnManager(
                on_turn_complete=self.on_handsfree_speech,
                on_speech_onset=self.on_speech_onset,
                speech_detector=selected_detector,
                on_armed=self.on_mic_armed,
            )
            selected = args.speech_detector if self.turn_manager.detector.speech_detector is not None else "loudness"
        print(f"Speech-start detector: {selected}", flush=True)
        print(f"Echo cancellation: {'available' if voice._echo_audio.available else 'unavailable'}", flush=True)
        self.session_log.append("session_started", interface="ui" if args.ui else "console",
                                speech_detector=selected, settings_path=args.settings)

    def submit_response(self, fn, token=None):
        with self.turn_manager.response_lock:
            token = self.turn_manager.begin_response() if token is None else token
            if token is None:
                return False
            self._active_response_token = token

            def execute(seq):
                self.turn_manager.wait_paused()
                if not self.worker.is_superseded(seq) and not self.shutdown.is_set():
                    fn(seq)

            def finished():
                with self.turn_manager.response_lock:
                    self.turn_manager.finish_response(token)
                    if self._active_response_token == token:
                        self._active_response_token = None
                    else:
                        return
                    if not self.shutdown.is_set():
                        if voice._is_muted:
                            self.bridge.emit_state("idle", "Muted")
                        else:
                            self.bridge.emit_state("idle" if self.args.ui else "listening")
            try:
                self.worker.submit(execute, on_done=finished)
            except Exception:
                finished()
                raise
            return True

    def on_handsfree_speech(self, audio_f32: np.ndarray, token):
        if token is None:
            return
        turn_id = self._capture_turn_id or self.session_log.new_turn_id()
        self._capture_turn_id = None
        if audio_f32 is None or not len(audio_f32):
            self.session_log.append("turn_outcome", turn_id, outcome="empty_capture")
            self.turn_manager.finish_response(token)
            if self.args.ui:
                self.bridge.emit_state("idle")
            return
        if self.args.ui:
            self.bridge.emit_state("thinking")

        def execute_turn(seq):
            self.bridge.emit_state("thinking", "Transcribing...")
            try:
                diagnostics = self.turn_manager.take_turn_diagnostics(audio_f32)
                print("[Audio turn] " + json.dumps(diagnostics, default=str, sort_keys=True), flush=True)
                self.session_log.append("audio_capture", turn_id, diagnostics=diagnostics)
            except Exception:
                pass
            stt_started = time.perf_counter()
            stt_result = "error"
            try:
                text = voice.transcribe_audio(audio_f32)
                stt_result = "text" if text else "empty"
            except Exception as exc:
                self.session_log.append("stt_error", turn_id, error={"type": type(exc).__name__, "message": str(exc)})
                raise
            finally:
                stt_elapsed = time.perf_counter() - stt_started
                print(f"[STT] elapsed={stt_elapsed:.2f}s result={stt_result}", flush=True)
            self.session_log.append("stt_result", turn_id, text=text or "", result=stt_result,
                                    elapsed_seconds=round(stt_elapsed, 3))
            if self.worker.is_superseded(seq):
                self.session_log.append("turn_outcome", turn_id, outcome="superseded")
                return
            print(f"\n[Transcribed]: {json.dumps(text or '', ensure_ascii=False)}", flush=True)
            if not text:
                self.session_log.append("turn_outcome", turn_id, outcome="empty_transcript")
                return
            if confirmation(text) is None and is_ignorable_filler(text):
                print("[Turn Guard]: Dropped an isolated filler transcript.", flush=True)
                self.session_log.append("turn_outcome", turn_id, outcome="filtered_filler")
                return
            voice.process_query(text, self.args.settings, bridge=self.bridge, conversation=self.conversation,
                                is_cancelled=lambda: self.worker.is_superseded(seq),
                                preserve_pending_on_cancel=self.args.ui, session_log=self.session_log,
                                turn_id=turn_id, input_source="voice")
        self.submit_response(execute_turn, token)

    def on_speech_onset(self):
        if not self.turn_manager.busy and not voice._is_muted:
            self.bridge.emit_state("listening", "Listening...")

    def on_mic_armed(self):
        if not voice._is_muted:
            self.bridge.emit_state("listening", "Listening...")

    def cancel_response(self):
        with self.turn_manager.response_lock:
            if self.conversation.pending and self.session_log:
                self.session_log.append("tool_confirmation", self.session_log.new_turn_id(), accepted=False,
                                        action_id=self.conversation.pending.get("id"), source="runtime_cancel")
            self.worker.cancel_current()
            self.turn_manager.set_muted(voice._is_muted)
            voice.stop_playback()
            clear_pending(self.conversation, self.bridge)

    def interrupt_response_for_capture(self):
        with self.turn_manager.response_lock:
            token = self._active_response_token
            if token is None:
                return
            self.worker.cancel_current()
            voice.stop_playback()
            self.turn_manager.finish_response(token, delay=0, preserve_capture=True)
            self._active_response_token = None

    def on_capture_error(self, _error):
        self.session_log.append("capture_error", self._capture_turn_id, error=str(_error))
        self.turn_manager.cancel_capture()
        self.bridge.emit_state("error")

    def stop_runtime(self):
        self.shutdown.set()
        self.turn_manager.stop()
        self.cancel_response()
        self.worker.stop()

    def submit_query(self, query, confirmation_id=None, input_source="ui"):
        turn_id = self.session_log.new_turn_id()
        return self.submit_response(lambda seq: voice.process_query(
            query, self.args.settings, auto_confirm=self.args.confirm, bridge=self.bridge,
            conversation=self.conversation, confirmation_id=confirmation_id,
            is_cancelled=lambda: self.worker.is_superseded(seq), session_log=self.session_log,
            turn_id=turn_id, input_source=input_source))

    def on_ui_action(self, action_data: Dict[str, Any]):
        act = action_data.get("action")
        if act == "window_closed":
            self.stop_runtime()
            return
        if self.shutdown.is_set():
            return
        if act == "ptt_start" and self.args.ui:
            voice._is_muted = False
            if self.turn_manager.begin_capture():
                self._capture_turn_id = self.session_log.new_turn_id()
                self.session_log.append("capture_started", self._capture_turn_id, source="push_to_talk")
                self.interrupt_response_for_capture()
                self.bridge.emit_state("held")
        elif act == "ptt_end" and self.args.ui:
            if self.turn_manager.end_capture():
                self.bridge.emit_state("grace")
        elif act == "ptt_cancel" and self.args.ui:
            self.session_log.append("turn_outcome", self._capture_turn_id, outcome="cancelled_before_transcription")
            self._capture_turn_id = None
            self.turn_manager.cancel_capture()
            self.bridge.emit_state("idle")
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
                self.submit_query("yes", action_id, input_source="ui_confirmation")
        elif act == "query":
            query = action_data.get("text", "")
            if query:
                self.submit_query(query, input_source="ui_text")

    def _warm_stt(self, seq):
        try:
            voice._stt_lifecycle.warm_cache()
            voice.get_stt()
            print("[STT] Model loaded and ready.", flush=True)
        except Exception as exc:
            print(f"[STT] Warm-up failed: {exc}", flush=True)
        if self.args.ui and not self.turn_manager.busy and not self.shutdown.is_set():
            self.bridge.emit_state("idle")

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
            self.submit_query(line, input_source="console")

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
                self.submit_query(self.args.query, input_source="cli")
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
    try:
        args.settings = prepare_settings(voice.LEKO_BIN, voice.REPO_ROOT, args.settings)
    except SettingsSetupError as exc:
        parser.exit(1, f"Settings setup failed: {exc}\n")
    voice.acquire_single_instance_lock()
    RuntimeController(args, selected_detector).run()
