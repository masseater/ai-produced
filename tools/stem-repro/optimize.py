import json

import scipy.optimize as so

from features import chroma, dist3
from search import excerpt
from surge import Surge

PARAMS = [
    "a_amp_eg_attack", "a_amp_eg_decay", "a_amp_eg_sustain", "a_amp_eg_release",
    "a_filter_1_cutoff", "a_filter_1_feg_mod_amount", "a_filter_eg_decay", "a_filter_eg_sustain",
]


def fit(song, part, patch, span, maxiter, popsize):
    ev, ref, dur = excerpt(song, part, *song.parts[part]["rescore_bars"])
    cref = chroma(ref)
    synth = Surge()
    synth.load(patch)
    prm = [synth.plugin.parameters[n] for n in PARAMS]
    x0 = [p.raw_value for p in prm]

    def loss(x):
        for p, v in zip(prm, x):
            p.raw_value = float(v)
        m = synth.render_mono(ev, dur, len(ref))
        if m is None:
            return 99.0
        s, e, c = dist3(m, ref, cref)
        return s + 0.5 * e + c

    l0 = loss(x0)
    bounds = [(max(0.0, v - span), min(1.0, v + span)) for v in x0]
    r = so.differential_evolution(loss, bounds, x0=x0, maxiter=maxiter, popsize=popsize, seed=0, polish=False, tol=1e-3)
    loss(r.x)
    return dict(part=part, patch=patch, loss0=round(l0, 3), loss=round(float(r.fun), 3),
                params=dict(zip(PARAMS, [float(v) for v in r.x])), params0=dict(zip(PARAMS, [float(v) for v in x0])))


def main(song, args):
    print(json.dumps(fit(song, args.part, args.patch, args.span, args.maxiter, args.popsize), ensure_ascii=False, indent=1))
