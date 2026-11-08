"""Feeds speaker reference and cleaned microphone PCM to the voice pipeline."""

import ctypes
import sys
import time
import threading
from pathlib import Path

import numpy as np

SAMPLE_RATE = 16_000
FRAME_SAMPLES = SAMPLE_RATE // 100


class EchoAudioProcessor:
    def __init__(self, library_path: Path | None = None):
        self.library = None
        self.handle = None
        self._lock = threading.Lock()
        self._render_tail = np.empty(0, dtype=np.float32)
        self._capture_tail = np.empty(0, dtype=np.float32)
        self._capture_output = np.empty(0, dtype=np.float32)
        path = library_path or self._library_path()
        try:
            self.library = ctypes.CDLL(str(path))
            self.library.leko_aec_create.argtypes = [ctypes.c_uint32]
            self.library.leko_aec_create.restype = ctypes.c_void_p
            self.library.leko_aec_destroy.argtypes = [ctypes.c_void_p]
            self.library.leko_aec_render.argtypes = [ctypes.c_void_p, ctypes.POINTER(ctypes.c_float), ctypes.c_size_t]
            self.library.leko_aec_capture.argtypes = [
                ctypes.c_void_p, ctypes.POINTER(ctypes.c_float), ctypes.POINTER(ctypes.c_float), ctypes.c_size_t
            ]
            self.library.leko_aec_render.restype = ctypes.c_int32
            self.library.leko_aec_capture.restype = ctypes.c_int32
            self.handle = self.library.leko_aec_create(SAMPLE_RATE)
        except (OSError, AttributeError) as exc:
            print(f"[Audio AEC unavailable; using playback mute]: {exc}", file=sys.stderr)

    @property
    def available(self) -> bool:
        return bool(self.handle)

    def _library_path(self) -> Path:
        root = Path(__file__).resolve().parents[2]
        if sys.platform == "darwin":
            name = "libleko_audio.dylib"
        elif sys.platform == "win32":
            name = "leko_audio.dll"
        else:
            name = "libleko_audio.so"
        for profile in ("release", "debug"):
            path = root / "target" / profile / name
            if path.exists():
                return path
        return root / "target" / "debug" / name

    def reset_buffers(self) -> None:
        with self._lock:
            self._render_tail = np.empty(0, dtype=np.float32)
            self._capture_tail = np.empty(0, dtype=np.float32)
            self._capture_output = np.empty(0, dtype=np.float32)

    def feed_render(self, samples: np.ndarray) -> bool:
        if not self.available:
            return False
        with self._lock:
            data = np.concatenate((self._render_tail, np.asarray(samples, dtype=np.float32)))
            complete = len(data) - len(data) % FRAME_SAMPLES
            for start in range(0, complete, FRAME_SAMPLES):
                frame = np.ascontiguousarray(data[start:start + FRAME_SAMPLES])
                if self.library.leko_aec_render(self.handle, frame.ctypes.data_as(ctypes.POINTER(ctypes.c_float)), len(frame)) != 0:
                    return False
            self._render_tail = data[complete:].copy()
        return True

    def clean_capture(self, samples_i16: np.ndarray) -> np.ndarray | None:
        if not self.available:
            return None
        samples = np.asarray(samples_i16, dtype=np.int16).astype(np.float32) / 32768.0
        with self._lock:
            pending = np.concatenate((self._capture_tail, samples))
            complete = len(pending) - len(pending) % FRAME_SAMPLES
            cleaned_frames = []
            for start in range(0, complete, FRAME_SAMPLES):
                frame = np.ascontiguousarray(pending[start:start + FRAME_SAMPLES])
                output = np.empty(FRAME_SAMPLES, dtype=np.float32)
                result = self.library.leko_aec_capture(
                    self.handle,
                    frame.ctypes.data_as(ctypes.POINTER(ctypes.c_float)),
                    output.ctypes.data_as(ctypes.POINTER(ctypes.c_float)),
                    FRAME_SAMPLES,
                )
                if result != 0:
                    return None
                cleaned_frames.append(output)
            if cleaned_frames:
                self._capture_output = np.concatenate((self._capture_output, *cleaned_frames))
            self._capture_tail = pending[complete:].copy()

            needed = len(samples)
            if len(self._capture_output) >= needed:
                cleaned = self._capture_output[:needed]
                self._capture_output = self._capture_output[needed:]
            else:
                cleaned = self._capture_output
                self._capture_output = np.empty(0, dtype=np.float32)
        return np.clip(cleaned * 32768.0, -32768, 32767).astype(np.int16)

    def close(self) -> None:
        with self._lock:
            if self.handle:
                self.library.leko_aec_destroy(self.handle)
                self.handle = None
            self._render_tail = np.empty(0, dtype=np.float32)
            self._capture_tail = np.empty(0, dtype=np.float32)
            self._capture_output = np.empty(0, dtype=np.float32)


class VoiceOutputPlayer:
    def __init__(self, processor: EchoAudioProcessor):
        self.processor = processor
        self.stream = None
        self._lock = threading.Lock()
        self._render_failed = False
        self._stop_generation = 0

    def play(self, pcm, sample_rate: int, is_cancelled=None) -> bool:
        if not self.processor.available:
            return False
        with self._lock:
            generation = self._stop_generation
        from math import gcd
        from scipy.signal import resample_poly
        total_duration_s = len(pcm) / sample_rate
        play_start_t = time.monotonic()
        was_interrupted = False
        import sounddevice as sd

        audio = self._mono_float(pcm)
        divisor = gcd(int(sample_rate), SAMPLE_RATE)
        render_reference = resample_poly(audio, SAMPLE_RATE // divisor, int(sample_rate) // divisor).astype(np.float32)
        self.processor.reset_buffers()
        position = 0
        self._render_failed = False

        def callback(outdata, frames, time_info, status):
            nonlocal position
            end = min(position + frames, len(audio))
            count = end - position
            outdata.fill(0)
            if count:
                outdata[:count, 0] = audio[position:end]
                ref_start = position * SAMPLE_RATE // sample_rate
                ref_end = end * SAMPLE_RATE // sample_rate
                if ref_end > ref_start and not self.processor.feed_render(render_reference[ref_start:ref_end]):
                    self._render_failed = True
                    raise sd.CallbackAbort
            position = end
            if position >= len(audio):
                raise sd.CallbackStop

        stream = None
        try:
            blocksize = int(sample_rate * 0.040)
            kwargs = {"blocksize": blocksize}
            import inspect
            sig = inspect.signature(sd.OutputStream.__init__)
            if "blocksize" not in sig.parameters and not any(p.kind == inspect.Parameter.VAR_KEYWORD for p in sig.parameters.values()):
                kwargs = {}
            stream = sd.OutputStream(samplerate=sample_rate, channels=1, dtype="float32", callback=callback, **kwargs)
            with self._lock:
                if generation != self._stop_generation or (is_cancelled and is_cancelled()):
                    stream.close()
                    return True
                self.stream = stream
                stream.start()
            while stream.active:
                if is_cancelled and is_cancelled():
                    was_interrupted = True
                    self.stop()
                    break
                time.sleep(0.02)
            with self._lock:
                stream.stop()
                stream.close()
                self.stream = None
            elapsed_s = time.monotonic() - play_start_t
            if was_interrupted or self._stop_generation != generation:
                print(f"[Maeve playback interrupted]: stopped at {elapsed_s:.2f}s / {total_duration_s:.2f}s", flush=True)
            else:
                print(f"[Maeve playback completed]: finished full {elapsed_s:.2f}s / {total_duration_s:.2f}s", flush=True)
            return not self._render_failed
        except Exception as exc:
            print(f"[Audio AEC playback unavailable; using playback mute]: {exc}", file=sys.stderr)
            self.stop()
            return False
        finally:
            with self._lock:
                if stream is not None:
                    try:
                        stream.close()
                    finally:
                        self.stream = None

    def stop(self) -> None:
        with self._lock:
            self._stop_generation += 1
            stream = self.stream
            if stream is not None:
                try:
                    stream.abort()
                except Exception:
                    pass

    @staticmethod
    def _mono_float(pcm) -> np.ndarray:
        audio = np.asarray(pcm, dtype=np.float32)
        if audio.ndim == 2:
            if audio.shape[0] <= 2 and audio.shape[1] > audio.shape[0]:
                audio = audio.mean(axis=0)
            else:
                audio = audio.mean(axis=1)
        return np.ascontiguousarray(np.clip(audio.reshape(-1), -1.0, 1.0))
