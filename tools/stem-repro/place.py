import json
from pathlib import Path

import numpy as np
import scipy.signal as sg
import soundfile as sf

from audio import SR, load, lr_db, lufs, mid_side, side_mid_db, stats
from features import dist
from match_eq import apply, curve


def compare(cand, ref):
    s, e = dist(cand.mean(1), ref.mean(1))
    return dict(spec_db=round(s, 2), env_db=round(e, 2), cand=stats(cand), ref=stats(ref), eq_needed=[int(round(v)) for v in curve(cand, ref)])


def widen(y, target):
    m, s = mid_side(y)
    cur = side_mid_db(y)
    if cur < target - 3:
        hp = sg.sosfilt(sg.butter(2, 200 / (SR / 2), "high", output="sos"), m)
        d = int(0.011 * SR)
        s = np.concatenate([np.zeros(d), hp[:-d]])
        s *= 10 ** (target / 20) * np.sqrt(np.mean(m**2)) / (np.sqrt(np.mean(s**2)) + 1e-12)
    else:
        s = s * 10 ** ((target - cur) / 20)
    return np.stack([m + s, m - s], 1)


def balance(y, target):
    d = (target - lr_db(y)) / 2
    return np.stack([y[:, 0] * 10 ** (d / 20), y[:, 1] * 10 ** (-d / 20)], 1)


def place(raw, ref):
    eq = curve(raw, ref)
    y = apply(raw, eq)
    target_sm = side_mid_db(ref)
    y = widen(y, target_sm) if target_sm > -60 else np.repeat(y.mean(1, keepdims=True), 2, 1)
    y = balance(y, lr_db(ref))
    a, b = lufs(ref), lufs(y)
    gain = 0.0 if a is None or b is None else a - b
    return y * 10 ** (gain / 20), dict(eq_applied_db=[int(round(v)) for v in eq], gain_db=round(gain, 1))


def main(song, args):
    n = song.frames()
    raw = load(args.raw, n)
    ref = song.stem(args.stem, n)
    before = compare(raw, ref)
    y, moves = place(raw, ref)
    Path(args.out).parent.mkdir(parents=True, exist_ok=True)
    sf.write(args.out, y, SR, subtype="FLOAT")
    print(json.dumps(dict(stem=args.stem, before=before, moves=moves, after=compare(y, ref)), ensure_ascii=False, indent=1))
