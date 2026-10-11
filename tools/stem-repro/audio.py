import numpy as np
import pyloudnorm as pyln
import soundfile as sf

SR = 44100


def load(path, n=None):
    x, sr = sf.read(path, always_2d=True)
    if sr != SR:
        raise ValueError(f"{path}: sample rate {sr}, expected {SR}")
    x = x[:, :2] if x.shape[1] > 1 else np.repeat(x, 2, 1)
    if n:
        x = np.pad(x, ((0, max(0, n - len(x))), (0, 0)))[:n]
    return x


def mid_side(x):
    return (x[:, 0] + x[:, 1]) / 2, (x[:, 0] - x[:, 1]) / 2


def lufs(x):
    try:
        v = pyln.Meter(SR).integrated_loudness(x)
    except ValueError:
        return None
    return round(float(v), 1) if np.isfinite(v) else None


def side_mid_db(x):
    m, s = mid_side(x)
    return round(float(20 * np.log10(np.sqrt(np.mean(s**2)) / (np.sqrt(np.mean(m**2)) + 1e-12) + 1e-9)), 1)


def lr_db(x):
    left = np.sqrt(np.mean(x[:, 0] ** 2))
    right = np.sqrt(np.mean(x[:, 1] ** 2))
    return round(float(20 * np.log10((left + 1e-12) / (right + 1e-12))), 1)


def stats(x):
    m, _ = mid_side(x)
    loud = np.abs(m) > 1e-4
    active = m[loud] if loud.any() else m
    r = np.sqrt(np.mean(active**2))
    peak = np.abs(x).max()
    return dict(lufs=lufs(x), crest=round(float(20 * np.log10(peak / (r + 1e-12))), 1), sm=side_mid_db(x))
