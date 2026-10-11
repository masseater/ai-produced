import json

import librosa
import numpy as np

from audio import SR
from midi_notes import notes


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
