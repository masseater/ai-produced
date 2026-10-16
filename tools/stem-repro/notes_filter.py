import json

import librosa
import numpy as np

from audio import SR
from midi_notes import notes, write


def supported(song, stem, lst, thr_db=-30, hop=512):
    m = song.stem(stem).mean(1)
    c = 20 * np.log10(np.abs(librosa.cqt(m, sr=SR, hop_length=hop, fmin=librosa.midi_to_hz(24), n_bins=84)) + 1e-9)
    ref = np.percentile(c.max(0), 99)
    out = []
    for s, e, p, _ in lst:
        i = int((s + 0.03) * SR / hop)
        b = p - 24
        if 0 <= b < 84 and i < c.shape[1]:
            lvl = c[b, i:i + 4].max() - ref
            if lvl > thr_db:
                out.append((s, e, p, int(np.clip(127 + 2.5 * lvl, 30, 127))))
    return out


def main(song, args):
    lst = notes(args.midi)
    if args.bars:
        lst = song.notes_in(lst, *args.bars)
    kept = supported(song, args.stem, lst, args.threshold)
    print(json.dumps(dict(stem=args.stem, input=len(lst), kept=len(kept), notes=kept)))


def splice(lst, other, ranges):
    inside = lambda t: any(a <= t < b for a, b in ranges)
    return sorted([n for n in lst if not inside(n[0])] + [n for n in other if inside(n[0])])


def grid(lst, step, fixed):
    out = {}
    for s, e, p, v in lst:
        g = round(s / step)
        t = g * step if abs(s / step - g) <= 0.3 else s
        out[(round(t, 4), p)] = max(out.get((round(t, 4), p), (t, e, p, 0)), (t, t + step if fixed else max(e, t + 0.03), p, v), key=lambda n: n[3])
    return sorted(out.values())


def main_edit(song, args):
    lst = notes(args.midi, args.track)
    if args.splice:
        lst = splice(lst, notes(args.splice), [(a * song.bar, b * song.bar) for a, b in zip(args.splice_bars[::2], args.splice_bars[1::2])])
    if args.grid:
        lst = grid(lst, song.bar / args.grid, args.fixed)
    write(args.out, lst, song.bpm)
    print(json.dumps(dict(notes=len(lst), out=args.out)))
