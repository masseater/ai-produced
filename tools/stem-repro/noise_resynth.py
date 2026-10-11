import numpy as np
import scipy.signal as sg
import soundfile as sf

from audio import SR, mid_side


def resynth(x, n=512, seed=0):
    rng = np.random.default_rng(seed)
    out = np.zeros_like(x)
    for ch, channel in enumerate(mid_side(x)):
        _, _, spec = sg.stft(channel, SR, nperseg=n, noverlap=n * 3 // 4)
        mag = np.abs(spec)
        noise = rng.standard_normal(spec.shape) + 1j * rng.standard_normal(spec.shape)
        _, y = sg.istft(mag * noise / np.abs(noise), SR, nperseg=n, noverlap=n * 3 // 4)
        out[:, ch] = y[:len(x)]
    return np.stack([out[:, 0] + out[:, 1], out[:, 0] - out[:, 1]], 1)


def main(song, args):
    sf.write(args.out, resynth(song.stem(args.stem), n=args.fft), SR, subtype="FLOAT")
