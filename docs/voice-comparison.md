# Voice interruption comparison

This is a test setup, not a claim that a detector is reliable. Run one preset at a time in the same room and at the same speaker volume. Ask the assistant to speak, then try interruptions, quiet speech, normal speech, and brief non-speech sounds. Record what happened without saving microphone audio or transcripts unless requested.

## Available now

From the project folder, use the same command with one of the detector settings:

```sh
/Users/calibro/.t3/scratch/phone-pilot/talk-venv/bin/python3 voice/assistant.py --ui --speech-detector loudness
/Users/calibro/.t3/scratch/phone-pilot/talk-venv/bin/python3 voice/assistant.py --ui --speech-detector silero
/Users/calibro/.t3/scratch/phone-pilot/talk-venv/bin/python3 voice/assistant.py --ui --speech-detector webrtc
/Users/calibro/.t3/scratch/phone-pilot/talk-venv/bin/python3 voice/assistant.py --ui --speech-detector rnnoise
/Users/calibro/.t3/scratch/phone-pilot/talk-venv/bin/python3 voice/assistant.py --ui --speech-detector ten
```

Launch only the selected line. The app prints the detector actually loaded and whether echo cancellation is available. If Silero fails, it prints a warning and uses loudness; do not count that as a Silero trial. All modes use the existing echo cancellation and turn-ending model. Restart the app to change modes. This setup does not automatically run any trial.

## Comparison order

1. Loudness, current short audio history — available.
2. Silero, current short audio history — available.
3. Loudness plus buffered transcription confirmation — experimental offline prototype only; not yet connected to the app.
4. WebRTC VAD — available; uses `webrtcvad-wheels==2.0.14` (MIT wrapper; WebRTC contains BSD-licensed code). Install into the voice environment with `uv pip install --python /Users/calibro/.t3/scratch/phone-pilot/talk-venv/bin/python3 webrtcvad-wheels==2.0.14`. If unavailable, the app exits before starting. A runtime detector failure disables listening and logs an error instead of silently using a different detector.
5. RNNoise — available on this macOS 15 arm64 machine using the native library in `pyrnnoise==0.4.3`. Install the pinned wheel with `uv pip install --python /Users/calibro/.t3/scratch/phone-pilot/talk-venv/bin/python3 --no-deps pyrnnoise==0.4.3`; the test uses its bundled native library directly, not the wrapper's optional CLI or its dependencies. The package wrapper is Apache-2.0 and the upstream RNNoise code is BSD-3-Clause. Upstream has an [open question about model-weight licensing](https://github.com/xiph/rnnoise/issues/284), so verify the bundled model's rights before distributing it. At startup a missing/incompatible library fails explicitly; inference failure disables listening and logs an error. The adapter resamples 16 kHz input to RNNoise's 48 kHz frames; its speech probability is used only for onset, not to change the echo-cancelled audio. This local experiment is not validated on other devices.
6. TEN VAD — available locally via a pinned official macOS universal binary. Its license adds restrictions to Apache 2.0, including on competition with Agora; review `voice/audio_io/models/TEN-LICENSE.txt` and `TEN-NOTICES.txt` before distribution. Missing/incompatible library fails before startup; inference failure disables listening and logs an error. See `voice/audio_io/models/TEN-MODEL.md` for the source revision and checksum. Live accuracy is unverified.
7. Apple voice processing — not yet integrated; this is an echo-cancellation alternative, not a speech detector.
8. Pause and resume on a false interruption, and adjustable confirmation delay — not yet integrated; these are interruption policies that can be tested with a detector.
9. Headphones — a physical test condition for echo, not a speech detector.

The separately researched approaches are candidates, not installed modes. Add or change a candidate only when its test is requested.

## Record each trial

| Item | Observation |
| --- | --- |
| Preset, echo status, and speaker/headphone setting | |
| Normal speech interrupts | |
| Quiet speech interrupts | |
| First words preserved | |
| Noise causes a false interruption | |
| Assistant hears its own voice | |
| Subjective delay and playback quality | |
| Errors and unexpected behavior | |

Until instrumented, perceived delay is subjective. Compare the same phrases, noises, and settings for each preset; single trials cannot establish reliability.

## Trial notes

- Loudness: user reported a slow or failed first interruption and a later successful interruption; possible self-hearing was uncertain. The log included an empty transcription and short fragments, but does not show the cause. Closing the UI left the assistant process running; it was stopped before the next trial.
- Silero: user reported two self-interruptions and rated the test a failure. The log recorded two exchanges without playback-stop reasons; the direct cause is unverified. The process was stopped before the next trial.
- Buffered transcription diagnostic: user reported that it did not respond to speech or interrupt, even after using the listen button. The isolated driver only plays a fixed prompt; its listen button unmutes and changes the UI, and it does not process conversation. Logs contain possible-interruption transcripts, but do not identify who produced them or prove an actual stop. Earlier attempts also encountered a thread error that was fixed before this trial. The diagnostic is not directly comparable with the full app.
- WebRTC VAD: user reported repeated listening or background triggers and no audible reply. The log recorded several transcripts and queries but no spoken response; it did not record why the responses did not play. The process was stopped.
- RNNoise: user reported repeated listening or background triggers and no usable spoken response. The log showed several transcripts and one attempted spoken reply; it did not establish whether that reply was audible or what caused the listening behavior. The process was stopped.
