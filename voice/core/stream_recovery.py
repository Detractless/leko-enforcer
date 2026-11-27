"""Recover sounddevice input streams across macOS CoreAudio and Bluetooth profile switches."""
from __future__ import annotations

import time
from typing import Callable, Optional


_BUILTIN_MIC_HINTS = ("built-in", "macbook", "imac", "mac mini", "mac studio", "mac pro")


def _refresh_portaudio(sd_module) -> None:
    terminate = getattr(sd_module, "_terminate", None)
    initialize = getattr(sd_module, "_initialize", None)
    if callable(terminate) and callable(initialize):
        try:
            terminate()
            initialize()
        except Exception:
            pass


def _resolve_device_pair(sd_module, devices):
    default = getattr(sd_module, "default", None)
    pair = getattr(default, "device", None)
    if pair is None:
        return None, None
    try:
        in_idx, out_idx = pair[0], pair[1]
    except (TypeError, IndexError):
        return None, None
    if not isinstance(in_idx, int) or not isinstance(out_idx, int):
        return None, None
    if in_idx < 0 or out_idx < 0 or in_idx >= len(devices) or out_idx >= len(devices):
        return None, None
    return devices[in_idx], devices[out_idx]


def resolve_input_device(sd_module) -> Optional[int]:
    """Avoid opening a 16 kHz Bluetooth HFP mic when the same headset is the output device."""
    query_devices = getattr(sd_module, "query_devices", None)
    if not callable(query_devices):
        return None
    try:
        devices = list(query_devices())
    except Exception:
        return None
    in_dev, out_dev = _resolve_device_pair(sd_module, devices)
    if not in_dev or not out_dev:
        return None
    in_name = str(in_dev.get("name") or "").strip()
    out_name = str(out_dev.get("name") or "").strip()
    in_sr = float(in_dev.get("default_samplerate") or 0.0)
    if not in_name or in_name != out_name or not (0.0 < in_sr <= 16000.0):
        return None

    candidates = []
    for idx, dev in enumerate(devices):
        if int(dev.get("max_input_channels") or 0) <= 0:
            continue
        name = str(dev.get("name") or "").strip()
        if not name or name == out_name:
            continue
        sr = float(dev.get("default_samplerate") or 0.0)
        if sr < 44100.0:
            continue
        lower = name.lower()
        is_builtin = any(hint in lower for hint in _BUILTIN_MIC_HINTS)
        dev_idx = dev.get("index")
        candidates.append((is_builtin, sr, dev_idx if isinstance(dev_idx, int) else idx))

    if not candidates:
        return None
    candidates.sort(key=lambda item: (item[0], item[1]), reverse=True)
    return candidates[0][2]


def open_input_stream(
    sd_module,
    *,
    can_refresh: Optional[Callable[[], bool]] = None,
    sleep_fn: Callable[[float], None] = time.sleep,
    **kwargs,
):
    """Open a RawInputStream, refreshing PortAudio and retrying transient AUHAL/Bluetooth errors."""
    explicit_device = "device" in kwargs and kwargs["device"] is not None
    attempts = [kwargs, kwargs, {**kwargs, "blocksize": 0}]
    last_exc = None
    for idx, params in enumerate(attempts):
        try:
            open_params = dict(params)
            if not explicit_device:
                preferred = resolve_input_device(sd_module)
                if preferred is not None:
                    open_params["device"] = preferred
            return sd_module.RawInputStream(**open_params)
        except Exception as exc:
            last_exc = exc
            if idx + 1 >= len(attempts):
                break
            if can_refresh is None or can_refresh():
                _refresh_portaudio(sd_module)
            sleep_fn(0.15 * (idx + 1))
    raise last_exc
