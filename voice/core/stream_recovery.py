"""Recover sounddevice input streams across macOS CoreAudio and Bluetooth profile switches."""
from __future__ import annotations

import time
from typing import Callable, Optional


def _refresh_portaudio(sd_module) -> None:
    terminate = getattr(sd_module, "_terminate", None)
    initialize = getattr(sd_module, "_initialize", None)
    if callable(terminate) and callable(initialize):
        try:
            terminate()
            initialize()
        except Exception:
            pass


def open_input_stream(
    sd_module,
    *,
    can_refresh: Optional[Callable[[], bool]] = None,
    sleep_fn: Callable[[float], None] = time.sleep,
    **kwargs,
):
    """Open a RawInputStream, refreshing PortAudio and retrying transient AUHAL/Bluetooth errors."""
    attempts = [kwargs, kwargs, {**kwargs, "blocksize": 0}]
    last_exc = None
    for idx, params in enumerate(attempts):
        try:
            return sd_module.RawInputStream(**params)
        except Exception as exc:
            last_exc = exc
            if idx + 1 >= len(attempts):
                break
            if can_refresh is None or can_refresh():
                _refresh_portaudio(sd_module)
            sleep_fn(0.15 * (idx + 1))
    raise last_exc
