import numpy as np
import scipy.signal as sg

from audio import SR
from features import CENTERS, ltas


def mono(x):
    return x.mean(1) if x.ndim > 1 else x


def curve(y, ref, limit=18):
    return np.clip(ltas(mono(ref)) - ltas(mono(y)), -limit, limit)


def apply(y, gains_db, ntaps=4095):
    f = np.linspace(0, SR / 2, 2049)
    g = np.interp(np.log(np.maximum(f, 1)), np.log(CENTERS), gains_db)
    h = sg.firwin2(ntaps, f, 10 ** (g / 20), fs=SR)
    if y.ndim == 1:
        return sg.fftconvolve(y, h, "same")
    return np.stack([sg.fftconvolve(y[:, c], h, "same") for c in range(y.shape[1])], 1)
