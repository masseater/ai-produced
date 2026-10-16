import json
from pathlib import Path

import librosa
import numpy as np
import soundfile as sf

from audio import SR, load
from midi_notes import notes as read_notes
from place import compare, place
from shape import bar_gain, beat_gain

HOP = 0.005


def frame_db(m, hop):
    k = len(m) // hop
    return 20 * np.log10(np.sqrt((m[:k * hop].reshape(k, hop) ** 2).mean(1)) + 1e-12)


def sounding(lst, ref, threshold=-30):
    e = frame_db(ref.mean(1), 441)
    top = np.percentile(e, 99.5)
    return [n for n in lst if e[int(n[0] * 100):int(n[0] * 100) + 5].max() - top >= threshold]


def onsets(lst, shift):
    out = []
    for t in sorted({round(n[0] + shift, 3) for n in lst}):
        if not out or t - out[-1] > 0.01:
            out.append(t)
    return np.array(out)


def gate(lst, shift, release_ms, n):
    g = np.zeros(n)
    t = np.arange(int(release_ms / 1000 * SR * 1.5)) / SR
    tail = 10 ** (-3 * t / (release_ms / 1000))
    a = int(0.001 * SR)
    for s, e, _, _ in lst:
        i = max(int((s + shift) * SR) - a, 0)
        j = min(int((e + shift) * SR), n)
        if j <= i:
            continue
        g[i:j] = 1.0
        k = min(len(tail), n - j)
        g[j:j + k] = np.maximum(g[j:j + k], tail[:k])
    return g


def note_profile(m, ons, hop, bins):
    acc = np.zeros(bins)
    k = 0
    for t in ons:
        i = int(t * SR)
        seg = m[i:i + hop * bins]
        if i >= 0 and len(seg) == hop * bins:
            acc += (seg.reshape(bins, hop) ** 2).mean(1)
            k += 1
    p = 10 * np.log10(acc / max(k, 1) + 1e-20)
    return p - p.max()


def per_note(y, ons, curve_of):
    gl = np.ones(len(y))
    for j, t in enumerate(ons):
        i = max(int(t * SR), 0)
        e = min(int(ons[j + 1] * SR) if j + 1 < len(ons) else i + int(0.5 * SR), len(y))
        if e > i:
            gl[i:e] = curve_of(j, i, e)
    return gl


def note_envelope(y, ref, ons, window=0.4, limit=24, iters=2):
    hop = int(HOP * SR)
    bins = int(window / HOP)
    total = np.zeros(bins)
    for _ in range(iters):
        g = np.clip(note_profile(ref.mean(1), ons, hop, bins) - note_profile(y.mean(1), ons, hop, bins), -limit, limit)
        g = np.convolve(np.pad(g, 1, mode="edge"), [0.25, 0.5, 0.25], "valid")
        total += g
        curve = 10 ** (np.interp(np.arange(hop * bins), np.arange(bins) * hop + hop / 2, g) / 20)

        def piece(j, i, e, curve=curve):
            n = e - i
            return curve[:n] if n <= len(curve) else np.concatenate([curve, np.full(n - len(curve), curve[-1])])

        y = y * per_note(y, ons, piece)[:, None]
    return y, [round(float(v), 1) for v in total[::4]]


def note_gain(y, ref, ons, limit=12):
    ym, rm = y.mean(1), ref.mean(1)
    gains = []

    def level(j, i, e):
        g = float(np.clip(20 * np.log10((np.sqrt(np.mean(rm[i:e] ** 2)) + 1e-9) / (np.sqrt(np.mean(ym[i:e] ** 2)) + 1e-9)), -limit, limit))
        gains.append(g)
        return 10 ** (g / 20)

    gl = per_note(y, ons, level)
    f = int(0.003 * SR)
    gl = np.convolve(np.pad(gl, (f, f), mode="edge"), np.ones(2 * f + 1) / (2 * f + 1), "valid")[:len(y)]
    return y * gl[:, None], dict(mean_db=round(float(np.mean(gains)), 2), sd_db=round(float(np.std(gains)), 2))


def silent_mask(ref, threshold=-80, fade_ms=2):
    m = ref.mean(1)
    hop = int(HOP * SR)
    g = np.zeros(len(m))
    for a, b in librosa.effects.split(m, top_db=-threshold, ref=1.0, frame_length=2 * hop, hop_length=hop):
        g[a:b] = 1
    f = int(fade_ms / 1000 * SR)
    return np.convolve(np.pad(g, f, mode="edge"), np.ones(2 * f + 1) / (2 * f + 1), "valid")[:, None]


def score(c):
    return c["spec_db"] + c["env_db"]


def candidates(song, raw, ref, lst, shift):
    ons = onsets(lst, shift)
    placed = place(raw, ref)[0]
    out = []
    for pre in [None, 10, 30, 80]:
        y0 = placed if pre is None else place(raw * gate(lst, shift, pre, len(raw))[:, None], ref)[0]
        for env in [False, True]:
            y1, moves = (note_envelope(y0, ref, ons) if env else (y0, None))
            barred = bar_gain(song, y1, ref)[0]
            for post, z in [("", y1), ("bars", barred), ("bars+beat", beat_gain(song, barred, ref)[0])]:
                if env or post:
                    z = place(z, ref, 1)[0]
                how = "+".join(x for x in ["place", f"gate{pre}" if pre else "", "noteenv" if env else "", post] if x)
                out.append((score(compare(z, ref)), how, z, dict(note_env_db=moves)))
    for _, how, z, moves in sorted(out, key=lambda r: r[0])[:3]:
        zn, gains = note_gain(z, ref, ons)
        zn = place(zn, ref, 1)[0]
        out.append((score(compare(zn, ref)), how + "+notegain", zn, dict(moves, note_gain=gains)))
    mask = silent_mask(ref)
    for _, how, z, moves in list(out):
        zm = place(z * mask, ref, 1)[0] * mask
        out.append((score(compare(zm, ref)), how + "+silence", zm, moves))
    return out


def main(song, args):
    n = song.frames()
    raw = load(args.raw, n)
    ref = song.stem(args.stem, n)
    lst = read_notes(args.midi) if args.midi else song.part_notes(args.part)
    kept = sounding(lst, ref)
    out = candidates(song, raw, ref, kept, args.shift_ms / 1000)
    best, how, z, moves = min(out, key=lambda r: r[0])
    Path(args.out).parent.mkdir(parents=True, exist_ok=True)
    sf.write(args.out, z, SR, subtype="FLOAT")
    print(json.dumps(dict(
        stem=args.stem,
        notes=len(lst),
        dropped_silent=len(lst) - len(kept),
        chosen=how,
        moves=moves,
        after=compare(z, ref),
        alternatives={h: round(s, 2) for s, h, _, _ in sorted(out, key=lambda r: r[0])},
    ), ensure_ascii=False, indent=1))
