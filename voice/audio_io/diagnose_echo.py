"""Compare playback paths without transcription, command execution, or saved mic audio."""

import json
from collections import Counter
import sys
import time
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import assistant as voice
from audio_io.echo_canceller import EchoAudioProcessor, VoiceOutputPlayer


def _state(name):
    """Fresh measurement state for one case."""
    return {
        "events": [],
        "timings": {"input": [], "output": []},
        "statuses": [],
        "stream_details": [],
        "frame_sizes": {"input": Counter(), "output": Counter()},
        "energy": {"raw": 0.0, "cleaned": 0.0, "samples": 0},
        "start": time.monotonic(),
    }


def _wrap_stream(factory, direction, rate, state):
    def create(*args, **kwargs):
        callback = kwargs.get("callback")
        sample_rate = kwargs.get("samplerate", rate)
        start = state["start"]
        if callback is not None:
            def measured(data, frames, clock, status):
                began = time.perf_counter()
                state["frame_sizes"][direction][frames] += 1
                if status:
                    state["statuses"].append({"direction": direction, "status": str(status),
                                              "seconds": round(time.monotonic() - start, 4)})
                try:
                    return callback(data, frames, clock, status)
                finally:
                    state["timings"][direction].append((time.perf_counter() - began, frames / sample_rate))
            kwargs["callback"] = measured
        stream = factory(*args, **kwargs)
        state["stream_details"].append({"direction": direction, "sample_rate": stream.samplerate,
                                        "blocksize": stream.blocksize, "latency": stream.latency})
        return stream
    return create


def _measure_energy(voice, energy):
    """Wrap clean_capture to accumulate raw and cleaned energy while playback runs."""
    clean_capture = voice._echo_audio.clean_capture

    def measured_capture(samples):
        cleaned = clean_capture(samples)
        if voice._playback_active and cleaned is not None:
            raw = samples.astype(np.float32) / 32768.0
            result = cleaned.astype(np.float32) / 32768.0
            energy["raw"] += float(np.dot(raw, raw))
            energy["cleaned"] += float(np.dot(result, result))
            energy["samples"] += len(raw)
        return cleaned

    return measured_capture


def _onset_handler(name, voice, event):
    def onset():
        event("speech_onset", during_playback=voice._playback_active)
        if name == "cancel" and voice._playback_active:
            event("stop_requested", reason="speech_onset")
            voice.stop_playback()

    return onset


def _playback(name, pcm, rate, voice):
    """Play through the right path; returns (started_seconds, success)."""
    if name == "baseline":
        voice.sd.play(pcm, samplerate=rate)
        voice.sd.wait()
        return True
    voice.set_echo_cancellation_enabled(True)
    voice.set_playback_state(True)
    success = voice._output_player.play(pcm, rate)
    voice.set_playback_state(False)
    return success


def _callback_timing(timings, frame_sizes):
    summary = {}
    for direction, samples in timings.items():
        summary[direction] = {
            "callbacks": len(samples),
            "max_ms": round(max((t for t, _ in samples), default=0) * 1000, 3),
            "over_budget": sum(t > budget for t, budget in samples),
            "frame_sizes": dict(frame_sizes[direction]),
        }
    return summary


def _event_recorder(state):
    def event(kind, **details):
        state["events"].append({
            "seconds": round(time.monotonic() - state["start"], 4),
            "event": kind,
            **details,
        })
    return event


def _wrap_audio_streams(rate, state):
    original_input = voice.sd.RawInputStream
    original_output = voice.sd.OutputStream
    voice.sd.RawInputStream = _wrap_stream(original_input, "input", rate, state)
    voice.sd.OutputStream = _wrap_stream(original_output, "output", rate, state)
    return original_input, original_output


def _prepare_audio(energy):
    voice._echo_audio.close()
    voice._echo_audio = EchoAudioProcessor()
    voice._output_player = VoiceOutputPlayer(voice._echo_audio)
    voice._echo_audio.clean_capture = _measure_energy(voice, energy)
    return voice._echo_audio


def _create_case_manager(name, event, echo_audio):
    if name == "baseline":
        return None
    if not echo_audio.available:
        raise RuntimeError("Native AEC library unavailable")
    onset = _onset_handler(name, voice, event)
    return voice.HandsfreeTurnManager(
        lambda audio, merger: event("turn_complete", samples=len(audio)), onset
    )


def _start_case_manager(manager):
    if manager is None:
        return
    manager.start()
    time.sleep(0.5)


def _play_case(name, pcm, rate, event):
    event("playback_start", expected_seconds=len(pcm) / rate)
    began = time.monotonic()
    success = _playback(name, pcm, rate, voice)
    event("playback_return", elapsed_seconds=time.monotonic() - began, returned_success=success)
    time.sleep(0.8)


def _cleanup_case(manager, original_streams):
    if manager is not None:
        manager.stop()
    voice._output_player.stop()
    voice._echo_audio.close()
    voice.sd.RawInputStream, voice.sd.OutputStream = original_streams


def run_case(name, pcm, rate):
    state = _state(name)
    event = _event_recorder(state)
    original_streams = _wrap_audio_streams(rate, state)
    manager = None
    try:
        echo_audio = _prepare_audio(state["energy"])
        manager = _create_case_manager(name, event, echo_audio)
        _start_case_manager(manager)
        _play_case(name, pcm, rate, event)
    finally:
        _cleanup_case(manager, original_streams)

    count = max(state["energy"]["samples"], 1)
    return {
        "case": name,
        "events": state["events"],
        "statuses": state["statuses"],
        "streams": state["stream_details"],
        "callback_timing": _callback_timing(state["timings"], state["frame_sizes"]),
        "playback_mic_rms": {
            key: (state["energy"][key] / count) ** 0.5 for key in ("raw", "cleaned")
        },
    }


def main():
    tts = voice.get_tts()
    pcm = VoiceOutputPlayer._mono_float(tts.synth(
        "This is an audio check. The same sentence should play smoothly from beginning to end.",
        voice=voice.VOICE_NAME,
    ))
    results = []
    for name in ("baseline", "observe", "cancel"):
        print(f"Audio comparison: {name}", flush=True)
        results.append(run_case(name, pcm, int(tts.sample_rate)))
        time.sleep(0.5)
    path = Path("/tmp/leko-audio-diagnosis.json")
    path.write_text(json.dumps(results, indent=2))
    print(json.dumps(results, indent=2))
    print(f"Timing report: {path}")


if __name__ == "__main__":
    main()
