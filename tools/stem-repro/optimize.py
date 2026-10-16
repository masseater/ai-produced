import json

import numpy as np
import scipy.optimize as so

from features import chroma, dist3
from midi_notes import events, shifted
from note_shape import note_profile
from surge import Surge

PARAMS = [
    "a_amp_eg_attack", "a_amp_eg_decay", "a_amp_eg_sustain", "a_amp_eg_release",
    "a_filter_1_cutoff", "a_filter_1_feg_mod_amount", "a_filter_eg_decay", "a_filter_eg_sustain",
]
SHIFT_MS = 12


def note_env(m, ons):
    return np.maximum(note_profile(m, ons, 220, 80), -50)


def fit(song, part, patch, maxiter, popsize):
    b0, b1 = song.parts[part]["rescore_bars"]
    t0, t1 = b0 * song.bar, b1 * song.bar
    lst = song.notes_in(song.part_notes(part), b0, b1)
    ref = song.bars(song.stem(part), b0, b1).mean(1)
    ons = [n[0] - t0 for n in lst]
    ne_ref, cref = note_env(ref, ons), chroma(ref)
    synth = Surge()
    synth.load(patch)
    prm = [synth.plugin.parameters[n] for n in PARAMS]
    x0 = [p.raw_value for p in prm] + [0.0]

    def terms(x):
        for p, v in zip(prm, x[:-1]):
            p.raw_value = float(v)
        sh = x[-1] / 1000
        m = synth.render_mono(events(shifted(lst, sh), t0, t1), t1 - t0, len(ref))
        if m is None:
            return None
        s, e, c = dist3(m, ref, cref)
        return [round(float(v), 3) for v in (s, e, c, np.mean(np.abs(note_env(m, ons) - ne_ref)))]

    def loss(x):
        t = terms(x)
        return 99.0 if t is None else float(sum(t))

    before = terms(x0)
    r = so.differential_evolution(loss, [(0.0, 1.0)] * len(PARAMS) + [(-SHIFT_MS, SHIFT_MS)], x0=x0, maxiter=maxiter, popsize=popsize, seed=1, polish=False, tol=1e-4)
    return dict(
        part=part,
        patch=patch,
        spec_env_chroma_noteenv_before=before,
        spec_env_chroma_noteenv=terms(r.x),
        loss=round(float(r.fun), 3),
        shift_ms=round(float(r.x[-1]), 2),
        params=dict(zip(PARAMS, [float(v) for v in r.x[:-1]])),
    )


def main(song, args):
    print(json.dumps(fit(song, args.part, args.patch, args.maxiter, args.popsize), ensure_ascii=False, indent=1))
