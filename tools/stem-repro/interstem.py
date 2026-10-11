import json
from pathlib import Path

import numpy as np
import scipy.signal as sg

from audio import SR, load, lr_db, lufs, side_mid_db
from features import CENTERS, band_power

SILENT = 10 ** (-60 / 20)


def stems_of(song, repro_dir):
    n = song.frames()
    if repro_dir is None:
        return {name: song.stem(name, n) for name in song.stem_names}
    return {name: load(Path(repro_dir) / f"{name}.wav", n) for name in song.stem_names}


def band_owner(song, stems, section):
    bp = {n: band_power(song.section(x, section).mean(1)) for n, x in stems.items()}
    tot = sum(bp.values()) + 1e-20
    rows = []
    for i, c in enumerate(CENTERS):
        top = sorted(((bp[n][i] / tot[i], n) for n in bp), reverse=True)[:3]
        rows.append([int(c), [[n, round(float(100 * v))] for v, n in top]])
    return rows


def beat_phase(song, stems, section, bins=48):
    out = {}
    beat = int(song.beat * SR)
    hop = beat // bins
    for n, x in stems.items():
        m = song.section(x, section).mean(1)
        if np.sqrt(np.mean(m**2)) < SILENT:
            out[n] = None
            continue
        acc = np.zeros(bins)
        k = 0
        for b in range(len(m) // beat - 1):
            acc += (m[b * beat:b * beat + hop * bins].reshape(bins, hop) ** 2).mean(1)
            k += 1
        e = 10 * np.log10(acc / max(k, 1) + 1e-20)
        out[n] = [round(float(v), 1) for v in (e - e.max())[::4]]
    return out


def low_overlap(song, stems, section, kick, bass, cutoff=150):
    sos = sg.butter(4, cutoff / (SR / 2), output="sos")
    hop = 441

    def energy(name):
        y = sg.sosfilt(sos, song.section(stems[name], section).mean(1))
        return np.array([np.mean(y[i:i + hop] ** 2) for i in range(0, len(y) - hop, hop)])

    ke, be = energy(kick), energy(bass)
    corr = np.corrcoef(10 * np.log10(ke + 1e-12), 10 * np.log10(be + 1e-12))[0, 1]
    return dict(env_corr=round(float(corr), 2), kick_share_pct=round(float(100 * ke.sum() / (ke.sum() + be.sum())), 1))


def stereo(song, stems, section):
    out = {}
    for n, x in stems.items():
        seg = song.section(x, section)
        seg = seg if np.sqrt(np.mean(seg**2)) > SILENT else x
        out[n] = dict(side_mid_db=side_mid_db(seg), lr_db=lr_db(seg))
    return out


def section_balance(song, stems):
    out = {}
    for s in song.sections:
        segs = {n: song.section(x, s) for n, x in stems.items()}
        mix = lufs(sum(segs.values()))
        row = dict(mix_lufs=mix)
        for n, seg in segs.items():
            v = lufs(seg)
            row[n] = None if v is None or mix is None or v < -70 else round(v - mix, 1)
        out[s] = row
    return out


def analyze(song, stems, section, kick, bass):
    return dict(
        band_owner=band_owner(song, stems, section),
        beat_phase_db=beat_phase(song, stems, section),
        low_kick_vs_bass=low_overlap(song, stems, section, kick, bass),
        stereo=stereo(song, stems, section),
        section_balance_rel_mix=section_balance(song, stems),
    )


def main(song, args):
    result = dict(orig=analyze(song, stems_of(song, None), args.section, args.kick, args.bass))
    if args.repro:
        result["repro"] = analyze(song, stems_of(song, args.repro), args.section, args.kick, args.bass)
    print(json.dumps(result, ensure_ascii=False, indent=1))
