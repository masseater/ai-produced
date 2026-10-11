import numpy as np
import scipy.signal as sg
import soundfile as sf

from audio import SR


def resynth(x, n=512, seed=0, smooth_bins=1):
    rng = np.random.default_rng(seed)
    ms = np.stack([(x[:, 0] + x[:, 1]) / 2, (x[:, 0] - x[:, 1]) / 2], 1)
    out = np.zeros_like(ms)
    for ch in range(2):
        _, _, spec = sg.stft(ms[:, ch], SR, nperseg=n, noverlap=n * 3 // 4)
        mag = np.abs(spec)
        if smooth_bins > 1:
            k = np.ones(smooth_bins) / smooth_bins
            mag = np.sqrt(np.apply_along_axis(lambda c: np.convolve(c, k, "same"), 0, mag**2))
        noise = rng.standard_normal(spec.shape) + 1j * rng.standard_normal(spec.shape)
        _, y = sg.istft(mag * noise / np.abs(noise), SR, nperseg=n, noverlap=n * 3 // 4)
        out[:, ch] = y[:len(x)]
    return np.stack([out[:, 0] + out[:, 1], out[:, 0] - out[:, 1]], 1)


def main(song, args):
    sf.write(args.out, resynth(song.stem(args.stem), n=args.fft), SR, subtype="FLOAT")
