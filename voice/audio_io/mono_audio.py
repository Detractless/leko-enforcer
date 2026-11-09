"""Coerces microphone audio to the finite mono float32 form the speech detectors need."""
import numpy as np


def finite_mono(audio, name: str) -> np.ndarray:
    """Returns audio as float32 mono, refusing anything else with a named sentence."""
    samples = np.asarray(audio, dtype=np.float32)
    if samples.ndim != 1 or not np.isfinite(samples).all():
        raise ValueError(f"{name} requires finite mono 16 kHz audio")
    return samples
