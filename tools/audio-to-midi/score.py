import argparse
import json

import mir_eval
import numpy as np
import pretty_midi


def load(path, track=None, programs=None):
    midi = pretty_midi.PrettyMIDI(path)
    out = []
    for i, inst in enumerate(midi.instruments):
        if inst.is_drum:
            continue
        if track is not None and i != track:
            continue
        if programs is not None and inst.program not in programs:
            continue
        out += [(n.start, max(n.end, n.start + 0.01), n.pitch) for n in inst.notes]
    return out


def arrays(notes, shift=0.0):
    intervals = np.array([[a + shift, b + shift] for a, b, _ in notes])
    pitches = np.array([pretty_midi.note_number_to_hz(p) for *_, p in notes])
    return intervals, pitches


def score(ref, est, shift, offset_ratio):
    p, r, f, _ = mir_eval.transcription.precision_recall_f1_overlap(
        *arrays(ref), *arrays(est, shift), onset_tolerance=0.05, offset_ratio=offset_ratio
    )
    return p, r, f


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("reference")
    parser.add_argument("estimate")
    parser.add_argument("--ref-instrument", type=int)
    parser.add_argument("--est-programs", type=lambda s: [int(x) for x in s.split(",")])
    parser.add_argument("--max-shift", type=float, default=0.1)
    args = parser.parse_args()
    ref = load(args.reference, track=args.ref_instrument)
    est = load(args.estimate, programs=args.est_programs)
    shifts = np.arange(-args.max_shift, args.max_shift + 1e-9, 0.005)
    shift = max(shifts, key=lambda s: score(ref, est, s, None)[2])
    p, r, f = score(ref, est, shift, None)
    _, _, f_off = score(ref, est, shift, 0.2)
    print(
        json.dumps(
            {
                "reference_notes": len(ref),
                "estimated_notes": len(est),
                "shift_ms": round(float(shift) * 1000),
                "precision": round(p, 3),
                "recall": round(r, 3),
                "f1": round(f, 3),
                "f1_with_offset": round(f_off, 3),
            }
        )
    )


if __name__ == "__main__":
    main()
