"""Recover sounddevice input and output streams across macOS CoreAudio and Bluetooth switches."""
from __future__ import annotations

import ctypes
import struct
import sys
import time
from typing import Callable, Optional


_BUILTIN_MIC_HINTS = ("built-in", "macbook", "imac", "mac mini", "mac studio", "mac pro")
BLUETOOTH_PREROLL_S = 0.65


class _AudioObjectPropertyAddress(ctypes.Structure):
    _fields_ = [("mSelector", ctypes.c_uint32), ("mScope", ctypes.c_uint32), ("mElement", ctypes.c_uint32)]


def _fourcc(code: str) -> int:
    return struct.unpack(">I", code.encode("ascii"))[0]


_BLUETOOTH_TRANSPORTS = frozenset((_fourcc("blue"), _fourcc("blea")))


def _coreaudio_default_output_id():
    """Return (coreaudio_lib, default_output_device_id) on macOS, or (None, None)."""
    if sys.platform != "darwin":
        return None, None
    try:
        ca = ctypes.CDLL("/System/Library/Frameworks/CoreAudio.framework/CoreAudio")
        ca.AudioObjectGetPropertyData.argtypes = [
            ctypes.c_uint32,
            ctypes.POINTER(_AudioObjectPropertyAddress),
            ctypes.c_uint32,
            ctypes.c_void_p,
            ctypes.POINTER(ctypes.c_uint32),
            ctypes.c_void_p,
        ]
        ca.AudioObjectGetPropertyData.restype = ctypes.c_int32
        dev_id = ctypes.c_uint32(0)
        size = ctypes.c_uint32(ctypes.sizeof(dev_id))
        dev_addr = _AudioObjectPropertyAddress(_fourcc("dOut"), _fourcc("glob"), 0)
        if ca.AudioObjectGetPropertyData(1, ctypes.byref(dev_addr), 0, None, ctypes.byref(size), ctypes.byref(dev_id)) != 0:
            return None, None
        return ca, dev_id.value
    except Exception:
        return None, None


def _coreaudio_default_output_name() -> Optional[str]:
    """Query macOS CoreAudio for the live default output device name."""
    ca, dev_id = _coreaudio_default_output_id()
    if ca is None or dev_id is None:
        return None
    try:
        cf = ctypes.CDLL("/System/Library/Frameworks/CoreFoundation.framework/CoreFoundation")
        cf.CFStringGetCString.argtypes = [ctypes.c_void_p, ctypes.c_char_p, ctypes.c_long, ctypes.c_uint32]
        cf.CFStringGetCString.restype = ctypes.c_bool
        cf.CFRelease.argtypes = [ctypes.c_void_p]

        cfstr = ctypes.c_void_p()
        name_size = ctypes.c_uint32(ctypes.sizeof(cfstr))
        name_addr = _AudioObjectPropertyAddress(_fourcc("lnam"), _fourcc("glob"), 0)
        if ca.AudioObjectGetPropertyData(dev_id, ctypes.byref(name_addr), 0, None, ctypes.byref(name_size), ctypes.byref(cfstr)) != 0 or not cfstr.value:
            return None
        try:
            buf = ctypes.create_string_buffer(512)
            if not cf.CFStringGetCString(cfstr, buf, len(buf), 0x08000100):
                return None
            return buf.value.decode("utf-8", errors="replace").strip() or None
        finally:
            cf.CFRelease(cfstr)
    except Exception:
        return None


def _coreaudio_default_output_transport() -> Optional[int]:
    """Query macOS CoreAudio for the default output device's transport type fourcc."""
    ca, dev_id = _coreaudio_default_output_id()
    if ca is None or dev_id is None:
        return None
    try:
        transport = ctypes.c_uint32(0)
        size = ctypes.c_uint32(ctypes.sizeof(transport))
        addr = _AudioObjectPropertyAddress(_fourcc("tran"), _fourcc("glob"), 0)
        if ca.AudioObjectGetPropertyData(dev_id, ctypes.byref(addr), 0, None, ctypes.byref(size), ctypes.byref(transport)) != 0:
            return None
        return transport.value
    except Exception:
        return None


def is_bluetooth_output(get_transport: Optional[Callable[[], Optional[int]]] = None) -> bool:
    """Return True when the active macOS default output device uses Bluetooth or BLE Audio."""
    transport = (get_transport or _coreaudio_default_output_transport)()
    return transport in _BLUETOOTH_TRANSPORTS


def pad_bluetooth_preroll(
    pcm,
    sample_rate: int,
    preroll_s: Optional[float] = None,
    is_bluetooth_fn: Optional[Callable[[], bool]] = None,
    warm_duration_fn: Optional[Callable[[], float]] = None,
    sd_module=None,
    keep_warm: Optional[bool] = None,
):
    """Prepend an inaudible non-zero pilot only for any remaining Bluetooth warm-up needed."""
    target_s = BLUETOOTH_PREROLL_S if preroll_s is None else preroll_s
    if pcm is None or not sample_rate or sample_rate <= 0 or target_s <= 0:
        return pcm
    try:
        n_samples = len(pcm)
        if n_samples == 0:
            return pcm
    except TypeError:
        return pcm
    if not (is_bluetooth_fn or is_bluetooth_output)():
        return pcm

    from .bluetooth_keepalive import bluetooth_warm_duration_s, make_bluetooth_pilot, warm_bluetooth_output

    use_keepalive = (is_bluetooth_fn is None and warm_duration_fn is None) if keep_warm is None else keep_warm
    warm_s = warm_duration_fn() if warm_duration_fn is not None else (bluetooth_warm_duration_s() if is_bluetooth_fn is None else 0.0)
    needed_s = max(0.0, float(target_s) - max(0.0, float(warm_s)))
    if use_keepalive:
        hold_s = (n_samples / float(sample_rate)) + needed_s + 20.0
        warm_bluetooth_output(sd_module=sd_module, hold_s=hold_s)
    if needed_s <= 0.0:
        return pcm

    import numpy as np

    arr = np.asarray(pcm, dtype=np.float32)
    channels = arr.shape[1] if arr.ndim == 2 else 1
    pilot = make_bluetooth_pilot(sample_rate, needed_s, channels=channels)
    if len(pilot) == 0:
        return pcm
    if arr.ndim == 2:
        return np.concatenate([pilot, arr], axis=0)
    return np.concatenate([pilot, arr.reshape(-1)])


def _refresh_portaudio(sd_module) -> None:
    try:
        from .bluetooth_keepalive import stop_bluetooth_keepalive
        stop_bluetooth_keepalive()
    except Exception:
        pass
    terminate = getattr(sd_module, "_terminate", None)
    initialize = getattr(sd_module, "_initialize", None)
    if callable(terminate) and callable(initialize):
        try:
            terminate()
            initialize()
        except Exception:
            pass


def _cached_output_name(sd_module) -> Optional[str]:
    default = getattr(sd_module, "default", None)
    pair = getattr(default, "device", None)
    query_devices = getattr(sd_module, "query_devices", None)
    if pair is None or not callable(query_devices):
        return None
    try:
        out_idx = pair[1]
        if not isinstance(out_idx, int) or out_idx < 0:
            return None
        devices = list(query_devices())
        if out_idx >= len(devices):
            return None
        return str(devices[out_idx].get("name") or "").strip() or None
    except Exception:
        return None


def sync_output_device(
    sd_module,
    *,
    can_refresh: Optional[Callable[[], bool]] = None,
    get_live_output_name: Optional[Callable[[], Optional[str]]] = None,
) -> bool:
    """Refresh PortAudio if macOS default output changed after startup."""
    terminate = getattr(sd_module, "_terminate", None)
    initialize = getattr(sd_module, "_initialize", None)
    if not (callable(terminate) and callable(initialize)):
        return False
    live_name = (get_live_output_name or _coreaudio_default_output_name)()
    if not live_name:
        return False
    if _cached_output_name(sd_module) == live_name:
        return False
    if can_refresh is not None and not can_refresh():
        return False
    _refresh_portaudio(sd_module)
    return True


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
