import json

import numpy as np
import soundfile as sf

from audio import SR, load
from place import compare, place

BINS = 48


def rms_db(x):
    return 20 * np.log10(np.sqrt(np.mean(x**2)) + 1e-12)


def bar_gain(song, y, ref, limit=12, silent=-70):
    step = int(song.bar * SR)
    n = int(np.ceil(len(y) / step))
    gains = []
    for i in range(n):
        a = rms_db(y[i * step:(i + 1) * step].mean(1))
        r = rms_db(ref[i * step:(i + 1) * step].mean(1))
        gains.append(-120.0 if r < silent else float(np.clip(r - a, -limit, limit)) if a > silent else 0.0)
    lin = np.repeat(10 ** (np.array(gains) / 20), step)[:len(y)]
    f = int(0.005 * SR)
    lin = np.convolve(np.pad(lin, (f, f), mode="edge"), np.ones(2 * f + 1) / (2 * f + 1), "valid")[:len(y)]
    return y * lin[:, None], [round(g, 1) for g in gains]


def beat_profile(song, m, beats):
    hop = int(song.beat * SR) // BINS
    acc = np.zeros(BINS)
    k = 0
    for i in beats:
        s = int(round(i * song.beat * SR))
        seg = m[s:s + hop * BINS]
        if len(seg) == hop * BINS:
            acc += (seg.reshape(BINS, hop) ** 2).mean(1)
            k += 1
    p = 10 * np.log10(acc / max(k, 1) + 1e-20)
    return p - p.max()


def beat_gain(song, y, ref, lo=-24, hi=6):
    rm = ref.mean(1)
    step = song.beat * SR
    level = np.array([rms_db(rm[int(i * step):int((i + 1) * step)]) for i in range(int(len(rm) / step))])
    beats = np.where(level > level.max() - 30)[0]
    g = np.clip(beat_profile(song, rm, beats) - beat_profile(song, y.mean(1), beats), lo, hi)
    g = np.convolve(np.concatenate([g[-1:], g, g[:1]]), [0.25, 0.5, 0.25], "valid")
    phase = (np.arange(len(y)) / step) % 1.0 * BINS
    x = np.concatenate([[-0.5], np.arange(BINS) + 0.5, [BINS + 0.5]])
    curve = np.interp(phase, x, np.concatenate([[g[-1]], g, [g[0]]]))
    return y * 10 ** (curve / 20)[:, None], [round(float(v), 1) for v in g]


def main(song, args):
    n = song.frames()
    raw = load(args.raw, n)
    ref = song.stem(args.stem, n)
    y, bars = bar_gain(song, raw, ref)
    phase = []
    if not args.bars_only:
        y, phase = beat_gain(song, y, ref)
    y, moves = place(y, ref)
    sf.write(args.out, y, SR, subtype="FLOAT")
    print(json.dumps(dict(stem=args.stem, bar_gain_db=bars, beat_phase_gain_db=phase, moves=moves, before=compare(raw, ref), after=compare(y, ref)), indent=1))
