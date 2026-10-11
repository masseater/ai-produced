import json

import numpy as np

from audio import SR, lufs, side_mid_db

BAND_EDGES = [20, 60, 120, 250, 500, 1000, 2000, 4000, 8000, 16000, 22050]


def bands(x):
    p = np.abs(np.fft.rfft(x * np.hanning(len(x)))) ** 2
    f = np.fft.rfftfreq(len(x), 1 / SR)
    tot = p.sum() + 1e-20
    return {f"{a}-{b}": round(float(10 * np.log10(p[(f >= a) & (f < b)].sum() / tot + 1e-12)), 1) for a, b in zip(BAND_EDGES, BAND_EDGES[1:])}


def report(song, name):
    x = song.stem(name)
    m = x.mean(1)
    step = int(song.bar * SR)
    bar_db = [20 * np.log10(np.sqrt(np.mean(m[i:i + step] ** 2)) + 1e-9) for i in range(0, len(m), step)]
    active = [i for i, v in enumerate(bar_db) if v > -45]
    seg = np.concatenate([m[i * step:(i + 1) * step] for i in active]) if active else m
    seg = seg[:SR * 30]
    peak = np.abs(x).max()
    return dict(
        lufs=lufs(x),
        peak_db=round(float(20 * np.log10(peak + 1e-12)), 1),
        crest_db=round(float(20 * np.log10(peak / (np.sqrt(np.mean(seg**2)) + 1e-12))), 1),
        side_mid_db=side_mid_db(x),
        activity="".join("#" if v > -45 else "+" if v > -60 else "." for v in bar_db),
        bands=bands(seg),
    )


def main(song, args):
    print(json.dumps({n: report(song, n) for n in song.stem_names}, ensure_ascii=False, indent=1))
