"""Own microphone streams off the callback thread; fence each capture generation."""
import collections
import math
import sys
import threading
import time
import numpy as np


def _empty_audio_stats():
    return {"samples": 0, "sum_squares": 0.0, "peak": 0.0, "clipped_samples": 0}


def _audio_stats(samples):
    values = np.asarray(samples).reshape(-1)
    stats = _empty_audio_stats()
    if not values.size:
        return stats
    if values.dtype == np.int16:
        absolute = np.abs(values.astype(np.int32))
        normalized = values.astype(np.float64) / 32768.0
        clipped = int(np.count_nonzero(absolute >= 32760))
    else:
        normalized = values.astype(np.float64)
        absolute = np.abs(normalized)
        clipped = int(np.count_nonzero(absolute >= 0.999))
    stats["samples"] = int(values.size)
    stats["sum_squares"] = float(np.dot(normalized, normalized))
    stats["peak"] = float(np.max(np.abs(normalized)))
    stats["clipped_samples"] = clipped
    return stats


def _merge_audio_stats(target, addition):
    target["samples"] += addition["samples"]
    target["sum_squares"] += addition["sum_squares"]
    target["peak"] = max(target["peak"], addition["peak"])
    target["clipped_samples"] += addition["clipped_samples"]


def _present_audio_stats(stats):
    count = stats["samples"]
    return {
        "samples": count,
        "rms": round(math.sqrt(stats["sum_squares"] / count), 6) if count else 0.0,
        "peak": round(stats["peak"], 6),
        "clipped_samples": stats["clipped_samples"],
    }


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
        self._pending_raw_buf = bytearray()
        self._raw_preroll_stats = collections.deque(
            maxlen=max(1, int(getattr(detector, "preroll_frames", 5)) + 1))
        self._active_raw_stats = None
        self._status_preroll = collections.deque(
            maxlen=max(2, 2 * (int(getattr(detector, "preroll_frames", 5)) + 1)))
        self._active_status_warnings = None
        self._last_turn_diagnostics = {}
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
        self._pending_raw_buf.clear()
        self._raw_preroll_stats.clear()
        self._active_raw_stats = None
        self._status_preroll.clear()
        self._active_status_warnings = None
        self.detector.reset()
        self.echo_audio.reset_buffers()

    @staticmethod
    def _status_warning(status):
        if status is None:
            return None
        names = ("input_overflow", "input_underflow", "output_overflow", "output_underflow", "priming_output")
        active = [name for name in names if getattr(status, name, False)]
        if active:
            return ",".join(active)
        try:
            return str(status).strip() if bool(status) else None
        except (TypeError, ValueError):
            return str(status).strip() or "unknown"

    def take_turn_diagnostics(self, audio_f32):
        """Return scalar turn diagnostics and discard the saved summary."""
        with self._cv:
            capture = self._last_turn_diagnostics
            self._last_turn_diagnostics = {}
        audio = np.asarray(audio_f32 if audio_f32 is not None else [], dtype=np.float32).reshape(-1)
        post_aec = _present_audio_stats(_audio_stats(audio))
        pre_aec = capture.get("pre_aec", _empty_audio_stats())
        return {
            "audio_seconds": round(len(audio) / self.sample_rate, 3),
            "aec_active": capture.get("aec_active", bool(self.echo_audio.available)),
            "pre_aec": _present_audio_stats(pre_aec),
            "post_aec": post_aec,
            "callback_warnings": capture.get("callback_warnings", []),
        }

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

    def _note_status_warning(self, warning):
        if self.detector.in_speech:
            if self._active_status_warnings is None:
                self._active_status_warnings = {}
            self._active_status_warnings[warning] = self._active_status_warnings.get(warning, 0) + 1
        else:
            self._status_preroll.append(warning)

    def _clean_samples(self, raw_samples):
        # Echo cancellation on the raw frame; a cancel failure closes the stream and stops the turn.
        samples = raw_samples
        if self.echo_audio.available:
            samples = self.echo_audio.clean_capture(samples)
            if samples is None:
                self.echo_audio.close()
                self.detector.set_echo_cancellation_enabled(False)
                self._clear()
                return None, True
        return samples, False

    def _track_raw_buffer(self, raw_samples, samples):
        if len(samples) == len(raw_samples):
            self._pending_raw_buf.extend(raw_samples.tobytes())
        else:
            self._raw_buf.clear()
            self._pending_raw_buf.clear()
            self._raw_preroll_stats.clear()
        self._raw_buf.extend(samples.tobytes())

    def _accumulate_chunk(self, raw_chunk, was_in_speech):
        if was_in_speech:
            if self._active_raw_stats is None:
                self._active_raw_stats = _empty_audio_stats()
            _merge_audio_stats(self._active_raw_stats, _audio_stats(raw_chunk))
        else:
            self._raw_preroll_stats.append(_audio_stats(raw_chunk))

    def _speech_onset(self):
        self._active_raw_stats = _empty_audio_stats()
        for stats in self._raw_preroll_stats:
            _merge_audio_stats(self._active_raw_stats, stats)
        self._raw_preroll_stats.clear()
        self._active_status_warnings = {}
        for flag in self._status_preroll:
            self._active_status_warnings[flag] = self._active_status_warnings.get(flag, 0) + 1
        self._status_preroll.clear()

    def _finish_turn(self, audio):
        raw_stats = self._active_raw_stats or _empty_audio_stats()
        status_warnings = self._active_status_warnings or {}
        self._last_turn_diagnostics = {
            "pre_aec": dict(raw_stats),
            "aec_active": bool(self.echo_audio.available),
            "callback_warnings": [
                {"flag": flag, "count": count}
                for flag, count in status_warnings.items()
            ],
        }
        token = self.begin_response()
        try:
            self.on_turn_complete(audio, token)
        except Exception:
            self.finish_response(token)
            raise

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
            warning = self._status_warning(status)
            if warning:
                self._note_status_warning(warning)
            raw_samples = np.frombuffer(bytes(indata), dtype=np.int16)
            samples, failed = self._clean_samples(raw_samples)
            if failed:
                return
            self._track_raw_buffer(raw_samples, samples)
            size = self.chunk_size * 2
            while len(self._raw_buf) >= size and len(self._pending_raw_buf) >= size:
                chunk = np.frombuffer(bytes(self._raw_buf[:size]), dtype=np.int16)
                raw_chunk = np.frombuffer(bytes(self._pending_raw_buf[:size]), dtype=np.int16)
                del self._raw_buf[:size]
                del self._pending_raw_buf[:size]
                was_in_speech = self.detector.in_speech
                self._accumulate_chunk(raw_chunk, was_in_speech)
                ended, audio = self.detector.feed(chunk)
                if not was_in_speech and self.detector.in_speech:
                    self._speech_onset()
                if ended and len(audio):
                    self._finish_turn(audio)
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
