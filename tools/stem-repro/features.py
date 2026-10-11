import librosa
import numpy as np

from audio import SR

EDGES = np.array([40 * 2 ** (i / 3) for i in range(29)])
CENTERS = np.sqrt(EDGES[:-1] * EDGES[1:])


def band_power(m, n=4096):
    n = n if len(m) >= 2 * n else 1024
    w = np.hanning(n)
    p = np.zeros(n // 2 + 1)
    k = 0
    for i in range(0, len(m) - n, n // 2):
        p += np.abs(np.fft.rfft(m[i:i + n] * w)) ** 2
        k += 1
    f = np.fft.rfftfreq(n, 1 / SR)
    return np.array([p[(f >= a) & (f < b)].sum() / max(k, 1) for a, b in zip(EDGES[:-1], EDGES[1:])])


def ltas(m):
    d = 10 * np.log10(band_power(m) + 1e-14)
    return d - d.max()


def env(m, hop=441):
    r = np.sqrt(np.convolve(m**2, np.ones(hop) / hop, "same")[::hop]) + 1e-9
    return 20 * np.log10(r)


def env_dist(ea, eb, floor=-50):
    ea = ea - ea.max()
    eb = eb - eb.max()
    mask = (eb > floor) | (ea > floor)
    return float(np.mean(np.abs(np.clip(ea[mask], floor, 0) - np.clip(eb[mask], floor, 0))))


def dist(a, b):
    la, lb = ltas(a), ltas(b)
    mask = lb > -60
    return float(np.mean(np.abs(la[mask] - lb[mask]))), env_dist(env(a), env(b))


def chroma(m):
    c = librosa.feature.chroma_stft(y=m.astype(float), sr=SR, n_fft=4096, hop_length=1024)
    v = c.mean(1)
    return v / (np.linalg.norm(v) + 1e-12)


def dist3(a, b, cb=None):
    s, e = dist(a, b)
    cb = chroma(b) if cb is None else cb
    return s, e, float(10 * (1 - np.dot(chroma(a), cb)))
