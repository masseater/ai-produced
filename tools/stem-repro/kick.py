import json

import librosa
import numpy as np
import scipy.optimize as so

from audio import SR
from features import env, env_dist

NAMES = ["f_end", "f_a", "tau_a", "f_b", "tau_b", "attack", "hold", "release", "click", "drive"]
BOUNDS = [(35, 48), (0, 600), (0.002, 0.08), (0, 120), (0.01, 0.3), (0.0005, 0.12), (0.05, 0.3), (0.005, 0.1), (0, 2), (0.5, 6)]


def kick(p, n):
    f_end, f_a, tau_a, f_b, tau_b, att, hold, rel, click, drive = p
    t = np.arange(n) / SR
    f = f_end + f_a * np.exp(-t / tau_a) + f_b * np.exp(-t / tau_b)
    amp = np.clip(t / max(att, 1e-4), 0, 1) * np.where(t < hold, 1, np.exp(-(t - hold) / max(rel, 1e-4)))
    y = np.tanh(drive * np.sin(2 * np.pi * np.cumsum(f) / SR) * amp) / np.tanh(drive)
    c = np.random.default_rng(0).standard_normal(n) * np.exp(-t / 0.004) * click
    return y + np.diff(np.concatenate([[0], c]))


def onsets(m):
    return librosa.onset.onset_detect(y=m, sr=SR, units="samples", delta=0.08, backtrack=True)


def mel(y):
    s = librosa.feature.melspectrogram(y=y, sr=SR, n_fft=2048, hop_length=256, n_mels=64, fmax=12000)
    return librosa.power_to_db(s + 1e-10)


def fit(song, stem, section, length_s, hits, maxiter, seed):
    m = song.section(song.stem(stem), section).mean(1)
    n = int(length_s * SR)
    shots = [m[o:o + n] for o in onsets(m) if o + n < len(m)][:hits]
    ref_mel = np.mean([mel(h) for h in shots], 0)
    ref_env = 10 * np.log10(np.mean([10 ** (env(h) / 10) for h in shots], 0))
    ref_rms = np.sqrt(np.mean(shots[0] ** 2))

    def loss(p):
        y = kick(p, n)
        y = y * ref_rms / (np.sqrt(np.mean(y**2)) + 1e-12)
        return float(np.mean(np.abs(np.maximum(mel(y), -80) - np.maximum(ref_mel, -80)))) + env_dist(env(y), ref_env)

    r = so.differential_evolution(loss, BOUNDS, maxiter=maxiter, popsize=10, seed=seed, polish=False)
    return dict(stem=stem, section=section, hits=len(shots), loss=round(float(r.fun), 3), params=dict(zip(NAMES, [float(v) for v in r.x])))


def main(song, args):
    print(json.dumps(fit(song, args.stem, args.section, args.length, args.hits, args.maxiter, args.seed), indent=1))
