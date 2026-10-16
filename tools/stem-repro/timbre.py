import json

import librosa
import numpy as np
from smstools.models import dftModel, harmonicModel, utilFunctions

from audio import SR, load
from features import dist
from midi_notes import notes as read_notes

HOP = 256
WINDOWS = [(0, 30), (30, 80), (80, 150), (150, 300)]


def f0_frames(lst, t0, n, voices):
    f = np.zeros((n, voices))
    for s, e, p, _ in lst:
        for i in range(max(int((s - t0) * SR / HOP), 0), min(int((e - t0) * SR / HOP) + 1, n)):
            row = sorted([v for v in f[i] if v > 0] + [librosa.midi_to_hz(p)])[:voices]
            f[i] = row + [0] * (voices - len(row))
    return f


def split(x, lst, t0, voices=4, harmonics=40):
    m = int(4 * SR / min(librosa.midi_to_hz(n[2]) for n in lst)) | 1
    nfft = int(2 ** np.ceil(np.log2(2 * m)))
    w = np.hamming(m)
    w /= w.sum()
    pad = np.concatenate([np.zeros(m // 2), x, np.zeros((m + 1) // 2)])
    frames = (len(pad) - m) // HOP
    f0 = f0_frames(lst, t0, frames + 1, voices)
    shape = (frames, harmonics * voices)
    hf, hm, hp = np.zeros(shape), np.full(shape, -200.0), np.zeros(shape)
    prev = np.zeros((voices, harmonics))
    for i in range(frames):
        c = (m + 1) // 2 + i * HOP
        mx, px = dftModel.dftAnal(pad[c - (m + 1) // 2:c + m // 2], w, nfft)
        loc = utilFunctions.peakDetection(mx, -100)
        iloc, imag, iph = utilFunctions.peakInterp(mx, px, loc)
        for v in range(voices):
            if f0[i, v] <= 0 or not len(iloc):
                prev[v] = 0
                continue
            cols = slice(v * harmonics, (v + 1) * harmonics)
            hf[i, cols], hm[i, cols], hp[i, cols] = harmonicModel.harmonicDetection(iloc * SR / nfft, imag, iph, f0[i, v], harmonics, prev[v], SR, 0.01)
            prev[v] = hf[i, cols]
    r = utilFunctions.sineSubtraction(x, 512, HOP, hf, hm, hp, SR)[:len(x)]
    r = np.pad(r, (0, len(x) - len(r)))
    return x - r, r, hm[:, :harmonics], m


def descriptors(x, ons, bins=60):
    s = np.abs(librosa.stft(x, n_fft=1024, hop_length=HOP))
    d = dict(
        centroid=librosa.feature.spectral_centroid(S=s, sr=SR)[0],
        rolloff=librosa.feature.spectral_rolloff(S=s, sr=SR, roll_percent=0.85)[0],
        flatness=librosa.feature.spectral_flatness(S=s)[0],
        flux=librosa.onset.onset_strength(S=librosa.amplitude_to_db(s), sr=SR, max_size=1),
    )
    return {k: np.median([v[i:i + bins] for i in (int(t * SR / HOP) for t in ons) if i + bins <= len(v)], 0) for k, v in d.items()}


def harmonic_profile(hm, ons, bins=60, n=12):
    rows = [hm[i:i + bins, :n] for i in (int(t * SR / HOP) for t in ons) if i + bins <= len(hm)]
    p = 10 * np.log10(np.mean(10 ** (np.array(rows) / 10), 0) + 1e-20)
    return np.maximum(p - p.max(), -60)


def windowed(v, fn):
    return {f"{a}-{b}ms": fn(v[int(a / 1000 * SR / HOP):int(b / 1000 * SR / HOP) + 1]) for a, b in WINDOWS}


def compare(ref, rep, lst, t0):
    rep = rep * np.sqrt(np.mean(ref**2)) / (np.sqrt(np.mean(rep**2)) + 1e-12)
    rh, rr, rm, m = split(ref, lst, t0)
    yh, yr, ym, _ = split(rep, lst, t0)
    score = lambda a: dict(zip(["spec_db", "env_db"], [round(v, 2) for v in dist(a, ref)]))
    share = lambda r, x: round(float(10 * np.log10(np.mean(r**2) / np.mean(x**2))), 1)
    ons = sorted({n[0] - t0 for n in lst})
    dr, dy = descriptors(ref, ons), descriptors(rep, ons)
    pr, py = harmonic_profile(rm, ons), harmonic_profile(ym, ons)
    return dict(
        window_ms=round(m / SR * 1000, 1),
        residual_share_db=dict(ref=share(rr, ref), repro=share(yr, rep)),
        swap=dict(repro=score(rep), ref_harmonics=score(rh + yr), ref_residual=score(yh + rr)),
        descriptors={k: windowed(np.stack([dr[k], dy[k]], 1), lambda v: [round(float(x), 3) for x in v.mean(0)]) for k in dr},
        harmonics_ref_db=windowed(pr, lambda v: [round(float(x), 1) for x in v.mean(0)]),
        harmonics_diff_db=windowed(py - pr, lambda v: [round(float(x), 1) for x in v.mean(0)]),
    )


def main(song, args):
    b0, b1 = args.bars
    t0 = b0 * song.bar
    lst = song.notes_in(read_notes(args.midi) if args.midi else song.part_notes(args.part), b0, b1)
    n = song.frames()
    ref = song.bars(song.stem(args.stem, n), b0, b1).mean(1)
    rep = song.bars(load(args.repro, n), b0, b1).mean(1)
    print(json.dumps(dict(stem=args.stem, bars=args.bars, notes=len(lst), **compare(ref, rep, lst, t0)), ensure_ascii=False, indent=1))
