import numpy as np
import scipy.signal as sg

from audio import SR
from features import CENTERS, ltas


def curve(y, ref, limit=18):
    return np.clip(ltas(ref.mean(1)) - ltas(y.mean(1)), -limit, limit)


def apply(y, gains_db, ntaps=4095):
    f = np.linspace(0, SR / 2, 2049)
    g = np.interp(np.log(np.maximum(f, 1)), np.log(CENTERS), gains_db)
    h = sg.minimum_phase(sg.firwin2(ntaps, f, 10 ** (g / 20), fs=SR), method="homomorphic", half=False)
    return np.stack([sg.fftconvolve(y[:, c], h)[:len(y)] for c in range(y.shape[1])], 1)


def match(y, ref, passes=2):
    total = 0
    for _ in range(passes):
        g = curve(y, ref)
        y = apply(y, g)
        total = total + g
    return y, total
