import json

import numpy as np
import scipy.signal as sg

from audio import SR, load, stats
from features import env
from match_eq import curve


def true_peak_db(x):
    return round(float(20 * np.log10(np.abs(sg.resample_poly(x, 4, 1, axis=0)).max() + 1e-12)), 2)


def align(a, b, max_lag=SR // 10):
    ma, mb = a.mean(1)[:SR * 30], b.mean(1)[:SR * 30]
    c = sg.correlate(mb, ma, mode="full", method="fft")
    mid = len(ma) - 1
    return int(np.argmax(c[mid - max_lag:mid + max_lag + 1]) - max_lag)


def transfer(src, dst, hop=441, step=3):
    es, ed = env(src.mean(1), hop), env(dst.mean(1), hop)
    n = min(len(es), len(ed))
    es, ed = es[:n], ed[:n]
    top = np.percentile(es, 99.9)
    rows = []
    for lo in np.arange(top - 30, top, step):
        sel = (es >= lo) & (es < lo + step)
        if sel.sum() > 20:
            rows.append(dict(in_db_rel_top=round(float(lo - top), 1), gain_db=round(float(np.median(ed[sel] - es[sel])), 2), frames=int(sel.sum())))
    return rows


def peak_reduction(src, dst, hop=441):
    def peaks(x):
        m = np.abs(x.mean(1))
        return np.array([m[i:i + hop].max() for i in range(0, len(m) - hop, hop)])

    ps, pd = peaks(src), peaks(dst)
    g = 20 * np.log10((np.median(np.abs(dst)) + 1e-12) / (np.median(np.abs(src)) + 1e-12))
    loud = ps > np.percentile(ps, 99)
    red = g - 20 * np.log10((pd[loud] + 1e-12) / (ps[loud] + 1e-12))
    return dict(median_gain_db=round(float(g), 2), loudest_1pct_extra_reduction_db=dict(p50=round(float(np.median(red)), 2), max=round(float(red.max()), 2)))


def measure(src, dst):
    lag = align(src, dst)
    if lag > 0:
        dst = dst[lag:]
    elif lag < 0:
        src = src[-lag:]
    n = min(len(src), len(dst))
    src, dst = src[:n], dst[:n]
    s, d = stats(src), stats(dst)
    return dict(
        lag_samples=lag,
        src=dict(s, true_peak_db=true_peak_db(src)),
        dst=dict(d, true_peak_db=true_peak_db(dst)),
        loudness_gain_db=None if s["lufs"] is None or d["lufs"] is None else round(d["lufs"] - s["lufs"], 1),
        eq_tilt_db=[round(float(v), 1) for v in curve(src, dst)],
        level_transfer=transfer(src, dst),
        peaks=peak_reduction(src, dst),
    )


def main(song, args):
    n = song.frames()
    premaster = load(song.file(song.cfg["premaster"]), n)
    out = {}
    if args.bus:
        out["bus_stem_sum_to_premaster"] = measure(sum(song.stem(name, n) for name in song.stem_names), premaster)
    if "master" in song.cfg:
        out["mastering_premaster_to_master"] = measure(premaster, load(song.file(song.cfg["master"]), n))
    print(json.dumps(out, ensure_ascii=False, indent=1))
