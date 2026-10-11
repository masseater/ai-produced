import json
from pathlib import Path

import librosa
import numpy as np
import scipy.optimize as so
import scipy.signal as sg
import soundfile as sf

from audio import SR
from features import dist
from match_eq import match

NAMES = ["f_end", "f_a", "tau_a", "f_b", "tau_b", "attack", "hold", "release", "drive", "knock_f", "knock_tau", "knock_lvl", "click", "click_tau", "phase"]
BOUNDS = [(30, 60), (0, 800), (0.001, 0.06), (0, 200), (0.01, 0.3), (0.0002, 0.02), (0.0, 0.35), (0.005, 0.15), (0.3, 6), (300, 3000), (0.001, 0.05), (0, 1.5), (0, 2), (0.0005, 0.01), (0, 2 * np.pi)]


def kick(p, n):
    f_end, f_a, tau_a, f_b, tau_b, att, hold, rel, drive, kf, kt, kl, cl, ct, ph = p
    t = np.arange(n) / SR
    f = f_end + f_a * np.exp(-t / tau_a) + f_b * np.exp(-t / tau_b)
    amp = np.clip(t / att, 0, 1) * np.where(t < hold, 1, np.exp(-(t - hold) / rel))
    body = np.sin(ph + 2 * np.pi * np.cumsum(f) / SR) * amp
    knock = kl * np.sin(2 * np.pi * kf * t) * np.exp(-t / kt)
    c = np.random.default_rng(0).standard_normal(n) * np.exp(-t / ct) * cl
    y = body + knock + np.diff(np.concatenate([[0], c]))
    return np.tanh(drive * y) / np.tanh(drive)


def template(m, length_s, hits):
    n = int(length_s * SR)
    starts = librosa.onset.onset_detect(y=m, sr=SR, units="samples", delta=0.08, backtrack=True)
    shots = [m[o:o + n] for o in starts if o + n < len(m)][:hits]
    first = shots[0]
    aligned = []
    for s in shots:
        c = sg.correlate(s, first, "full", method="fft")
        lag = int(np.argmax(c[n - 1 - 441:n + 441])) - 441
        aligned.append(np.roll(s, -lag))
    t = np.median(aligned, 0)
    start = max(int(np.argmax(np.abs(t) > 0.1 * np.abs(t).max())) - int(0.001 * SR), 0)
    return np.concatenate([t[start:], np.zeros(start)])


def spec(y, n):
    _, _, z = sg.stft(y, SR, nperseg=n, noverlap=n * 3 // 4)
    return 20 * np.log10(np.abs(z) + 1e-6)


def env2ms(y, h=88):
    k = len(y) // h
    e = 10 * np.log10((y[:k * h].reshape(k, h) ** 2).mean(1) + 1e-12)
    return np.maximum(e - e.max(), -60)


def fit(ref, maxiter, seed):
    n = len(ref)
    target = {k: spec(ref, k) for k in (256, 1024, 4096)}
    target_env = env2ms(ref)

    def loss(p):
        y = kick(p, n)
        y = y * np.sqrt(np.mean(ref**2)) / (np.sqrt(np.mean(y**2)) + 1e-12)
        s = np.mean([np.mean(np.abs(np.maximum(spec(y, k), r.max() - 80) - np.maximum(r, r.max() - 80))) for k, r in target.items()])
        return float(s + np.mean(np.abs(env2ms(y) - target_env)))

    r = so.differential_evolution(loss, BOUNDS, maxiter=maxiter, popsize=15, seed=seed, polish=True, tol=1e-6)
    return float(r.fun), [float(v) for v in r.x]


def hits(m, tmpl, grid_s, window_s=0.12):
    w = int(window_s * SR)
    t = tmpl[:w]
    out = []
    for b in range(int(len(m) / (grid_s * SR))):
        g = int(round(b * grid_s * SR))
        lo = max(0, g - 441)
        seg = m[lo:g + 441 + w]
        if len(seg) < w + 882 or np.sqrt(np.mean(seg**2)) < 1e-3:
            continue
        c = sg.correlate(seg, t, "valid", method="fft")
        k = int(np.argmax(c))
        gain = c[k] / np.dot(t, t)
        cc = c[k] / (np.linalg.norm(seg[k:k + w]) * np.linalg.norm(t) + 1e-12)
        if gain > 0.4 and cc > 0.5 and (not out or lo + k - out[-1][0] > 0.05 * SR):
            out.append((lo + k, float(gain)))
    return out


def shot(p, tmpl, n):
    k = kick(p, n)
    if np.dot(k[:len(tmpl)], tmpl) < 0:
        k = -k
    k *= np.sqrt(np.mean(tmpl**2)) / np.sqrt(np.mean(k[:len(tmpl)] ** 2))
    h = int(0.005 * SR)
    m = len(tmpl) // h * h
    er = np.sqrt((tmpl[:m].reshape(-1, h) ** 2).mean(1))
    ek = np.sqrt((k[:m].reshape(-1, h) ** 2).mean(1))
    corr = np.clip(20 * np.log10((er + 1e-9) / (ek + 1e-9)), -12, 12)
    corr = np.convolve(np.pad(corr, 1, mode="edge"), [0.25, 0.5, 0.25], "valid")
    gain = np.interp(np.arange(n), np.arange(len(corr)) * h + h / 2, corr)
    gain[m:] = gain[m - 1]
    return k * 10 ** (gain / 20), corr


def track(p, tmpl, found, n):
    k, corr = shot(p, tmpl, n)
    y = np.zeros(n)
    for i, (s, g) in enumerate(found):
        last = i + 1 == len(found)
        room = n - s if last else min(found[i + 1][0], n) - s
        seg = k[:room if last else min(len(tmpl), room)].copy()
        f = min(64, len(seg))
        seg[-f:] *= np.linspace(1, 0, f)
        y[s:s + len(seg)] += g * seg
    return np.stack([y, y], 1), corr


def main(song, args):
    m = song.section(song.stem(args.stem), args.section).mean(1)
    loss, p = fit(template(m, args.length, args.hits), args.maxiter, args.seed)
    print(json.dumps(dict(stem=args.stem, section=args.section, loss=round(loss, 3), params=dict(zip(NAMES, p))), indent=1))


def main_track(song, args):
    n = song.frames()
    ref = song.stem(args.stem, n)
    m = ref.mean(1)
    tmpl = template(song.section(ref, args.section).mean(1), args.length, args.hits)
    p = list(json.loads(Path(args.params).read_text())["params"].values())
    found = hits(m, tmpl, song.beat / 4)
    y, corr = track(p, tmpl, found, n)
    y, eq = match(y, ref, 3)
    Path(args.out).parent.mkdir(parents=True, exist_ok=True)
    sf.write(args.out, y, SR, subtype="FLOAT")
    s, e = dist(y.mean(1), m)
    gains = [g for _, g in found]
    print(json.dumps(dict(
        stem=args.stem,
        hits=len(found),
        hit_gain_p5_p50_p95=[round(float(v), 3) for v in np.percentile(gains, [5, 50, 95])],
        env_correction_db=[round(float(v), 1) for v in corr],
        eq_db=[int(round(v)) for v in eq],
        spec_db=round(s, 2),
        env_db=round(e, 2),
    ), indent=1))
