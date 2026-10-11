import itertools
import json
from pathlib import Path

import numpy as np
import pedalboard as pb
import scipy.signal as sg
import soundfile as sf

from audio import SR, load, lufs, stats
from features import env
from match_eq import apply, curve
from place import compare


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


def loudness_to(y, target):
    return y * 10 ** ((target - lufs(y)) / 20)


def fit_bus(stem_sum, premaster):
    target = lufs(premaster)
    want = stats(premaster)["crest"]
    x = loudness_to(stem_sum, target)
    peak = 20 * np.log10(np.abs(x).max())
    best = None
    for below, ratio, attack, release in itertools.product([3, 6, 9, 12, 15], [2, 4, 8], [0.5, 3, 10], [50, 150]):
        comp = pb.Compressor(threshold_db=peak - below, ratio=ratio, attack_ms=attack, release_ms=release)
        y = loudness_to(comp(x.T.astype(np.float32), SR).T.astype(float), target)
        c = compare(y, premaster)
        s = abs(c["cand"]["crest"] - want) + c["env_db"] / 2
        if best is None or s < best[0]:
            best = (s, dict(threshold_below_peak_db=below, ratio=ratio, attack_ms=attack, release_ms=release), y, c)
    return best[2], dict(compressor=best[1], peaks=peak_reduction(x, best[2]), vs_premaster=best[3])


def soft_clip(y, ceiling_db, knee, oversample=1):
    u = sg.resample_poly(y, oversample, 1, axis=0) if oversample > 1 else y
    c = 10 ** (ceiling_db / 20)
    v = u / c
    v = v / (1 + np.abs(v) ** knee) ** (1 / knee) * c
    return (sg.resample_poly(v, 1, oversample, axis=0) if oversample > 1 else v)[:len(y)]


def fit_master(mix, premaster, master):
    target = lufs(master)
    pre = apply(mix, curve(premaster, master))
    want = np.array([r["gain_db"] for r in transfer(premaster, master)])
    want_crest, want_tp = stats(master)["crest"], true_peak_db(master)
    best = None
    for ceiling, knee, oversample in itertools.product([-3, -2, -1.5, -1, -0.5, 0], [4, 8, 32], [1, 4]):
        g = target - lufs(pre)
        for _ in range(5):
            y = soft_clip(pre * 10 ** (g / 20), ceiling, knee, oversample)
            g += target - lufs(y)
        got = np.array([r["gain_db"] for r in transfer(mix, y)])
        k = min(len(got), len(want))
        s = abs(stats(y)["crest"] - want_crest) + np.max(np.abs(got[:k] - want[:k])) / 2 + abs(true_peak_db(y) - want_tp) / 2
        if best is None or s < best[0]:
            best = (s, dict(ceiling_db=ceiling, knee=knee, oversample=oversample, gain_db=round(float(g), 1)), y)
    y = best[2]
    return y, dict(clipper=best[1], true_peak_db=true_peak_db(y), level_transfer=transfer(mix, y), vs_master=compare(y, master))


def main_fit(song, args):
    n = song.frames()
    premaster = load(song.file(song.cfg["premaster"]), n)
    master = load(song.file(song.cfg["master"]), n)
    stem_sum = sum(load(p, n) for p in sorted(Path(args.stems_dir).glob("*.wav")))
    mix, bus = fit_bus(stem_sum, premaster)
    out, mst = fit_master(mix, premaster, master)
    d = Path(args.out)
    d.mkdir(parents=True, exist_ok=True)
    sf.write(d / "mix_noMst.wav", mix, SR, subtype="FLOAT")
    sf.write(d / "master.wav", out, SR, subtype="PCM_24")
    print(json.dumps(dict(bus=bus, master=mst), ensure_ascii=False, indent=1, default=float))
