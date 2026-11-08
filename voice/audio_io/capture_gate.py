"""Own microphone streams off the callback thread; fence each capture generation."""
import sys
import threading
import time
import numpy as np


class CaptureGate:
    def __init__(self, stream_factory, detector, echo_audio, on_turn_complete,
                 sample_rate=16000, chunk_size=2560, on_armed=None):
        self.stream_factory = stream_factory
        self.detector = detector
        self.echo_audio = echo_audio
        self.on_turn_complete = on_turn_complete
        self.on_armed = on_armed
        self._armed = False
        self.sample_rate, self.chunk_size = sample_rate, chunk_size
        self._cv = threading.Condition(threading.RLock())
        self.response_lock = self._cv
        self._raw_buf = bytearray()
        self._epoch = 0
        self._busy = None
        self._muted = False
        self._closed = False
        self._paused = threading.Event()
        self._paused.set()
        self._resume_at = 0
        self.is_running = False
        self.stream = None
        self._thread = None

    @property
    def busy(self):
        with self._cv:
            return self._busy is not None or self._closed

    def _clear(self):
        self._raw_buf.clear()
        self.detector.reset()
        self.echo_audio.reset_buffers()

    def begin_response(self):
        with self._cv:
            if self._busy is not None or self._closed:
                return None
            self._epoch += 1
            self._busy = self._epoch
            self._armed = False
            self._clear()
            self._cv.notify_all()
            return self._busy

    def finish_response(self, token, delay=None):
        with self._cv:
            if token != self._busy or token is None:
                return
            if delay is None:
                delay = 0.0 if self.echo_audio.available else 0.25
            self._busy = None
            self._clear()
            self._resume_at = time.monotonic() + delay
            self._cv.notify_all()

    def wait_paused(self):
        self._paused.wait()

    def set_muted(self, muted):
        with self._cv:
            self._muted = bool(muted)
            self._epoch += 1
            self._armed = False
            self._clear()
            self._cv.notify_all()

    def start(self):
        with self._cv:
            if self._closed or self._thread:
                return
            self.is_running = True
            self._thread = threading.Thread(target=self._control, name="capture-control", daemon=True)
            self._thread.start()

    def stop(self):
        with self._cv:
            self._closed = True
            self.is_running = False
            self._epoch += 1
            self._clear()
            self._cv.notify_all()
        if self._thread and threading.current_thread() is not self._thread:
            self._thread.join()

    def _allowed(self):
        return (self.is_running and not self._closed and self._busy is None
                and not self._muted and time.monotonic() >= self._resume_at)

    def _callback(self, epoch, indata, frames, time_info, status):
        # Gate before AEC, VAD, or pre-roll. Old streams can never feed a new turn.
        with self._cv:
            if epoch != self._epoch or not self._allowed():
                return
            if not self._armed:
                self._armed = True
                if self.on_armed:
                    try:
                        self.on_armed()
                    except Exception:
                        pass
            samples = np.frombuffer(bytes(indata), dtype=np.int16)
            if self.echo_audio.available:
                samples = self.echo_audio.clean_capture(samples)
                if samples is None:
                    self.echo_audio.close()
                    self.detector.set_echo_cancellation_enabled(False)
                    self._clear()
                    return
            self._raw_buf.extend(samples.tobytes())
            size = self.chunk_size * 2
            while len(self._raw_buf) >= size:
                chunk = np.frombuffer(bytes(self._raw_buf[:size]), dtype=np.int16)
                del self._raw_buf[:size]
                ended, audio = self.detector.feed(chunk)
                if ended and len(audio):
                    token = self.begin_response()
                    try:
                        self.on_turn_complete(audio, token)
                    except Exception:
                        self.finish_response(token)
                        raise
                    return

    def _control(self):
        while True:
            with self._cv:
                # Open the device as soon as a turn ends so its startup cost
                # overlaps the echo-decay window; audio stays gated in _allowed().
                while not self._closed and (self._busy is not None or self._muted):
                    self._cv.wait()
                if self._closed:
                    return
                self._epoch += 1
                epoch = self._epoch
                self._clear()
                self._paused.clear()
            stream = None
            try:
                stream = self.stream_factory(
                    samplerate=self.sample_rate, channels=1, dtype="int16", blocksize=1280,
                    callback=lambda *args, generation=epoch: self._callback(generation, *args))
                self.stream = stream
                stream.start()
                with self._cv:
                    while (epoch == self._epoch and not self._closed
                           and self._busy is None and not self._muted):
                        self._cv.wait()
            except Exception as exc:
                print(f"[Microphone error]: {exc}", file=sys.stderr)
                with self._cv:
                    self._muted = True
            finally:
                try:
                    if stream:
                        try:
                            stream.stop()
                        finally:
                            stream.close()
                except Exception as exc:
                    # Unknown device state: never reopen automatically.
                    print(f"[Microphone cleanup error]: {exc}", file=sys.stderr)
                    with self._cv:
                        self._closed = True
                self.stream = None
                self._paused.set()
