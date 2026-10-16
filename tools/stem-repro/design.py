import json
import os

import cma
import librosa
import numpy as np
import soundfile as sf

from audio import SR
from features import dist
from midi_notes import events, shifted
from midi_notes import notes as read_notes
from optimize import note_env
from surge import PATCHES, Surge

FILTERS = dict(off=0, lp12=0.025, lp24=0.05, ladder=0.085, vintage=0.295, obxd12=0.325, obxd24=0.355, k35=0.385, diode=0.445)
PARAMS = [
    "a_osc_1_shape", "a_osc_1_width_1", "a_osc_1_sub_mix",
    "a_filter_1_cutoff", "a_filter_1_resonance", "a_filter_1_feg_mod_amount",
    "a_filter_eg_attack", "a_filter_eg_decay", "a_filter_eg_sustain", "a_filter_eg_release",
    "a_amp_eg_attack", "a_amp_eg_decay", "a_amp_eg_sustain", "a_amp_eg_release",
    "a_waveshaper_drive",
]
NOISE = ["a_noise_volume", "a_noise_color", "a_osc_1_unison_detune"]
SHIFT_MS = 12
TEMPLATES = os.path.join(PATCHES, "patches_factory", "Templates")


def harmonics(m, ons, f0s, n=12, hop=0.01, win=0.04, frames=15, nfft=4096):
    w, h = int(win * SR), int(hop * SR)
    freqs = np.fft.rfftfreq(nfft, 1 / SR)
    acc, k = np.zeros((n, frames)), 0
    for s, f0 in zip(ons, f0s):
        i = int(s * SR)
        if i < 0 or i + (frames - 1) * h + w > len(m):
            continue
        seg = np.stack([m[i + j * h:i + j * h + w] for j in range(frames)]) * np.hanning(w)
        power = np.abs(np.fft.rfft(seg, nfft)).T ** 2
        acc += librosa.f0_harmonics(power, f0=np.full(frames, f0), freqs=freqs, harmonics=np.arange(1, n + 1))
        k += 1
    d = 10 * np.log10(acc.T / max(k, 1) + 1e-20)
    return np.maximum(d - d.max(), -60)


def fit(song, part, template, filt, bars, iters, noise, notes=None):
    b0, b1 = bars
    t0, t1 = b0 * song.bar, b1 * song.bar
    lst = song.notes_in(notes or song.part_notes(part), b0, b1)
    ref = song.bars(song.stem(part), b0, b1).mean(1)
    ons, f0s = [n[0] - t0 for n in lst], [librosa.midi_to_hz(n[2]) for n in lst]
    h_ref, ne_ref = harmonics(ref, ons, f0s), note_env(ref, ons)
    fixed = dict(a_filter_1_type=FILTERS[filt], a_waveshaper_type=0.02 if filt != "off" else 0.0)
    if noise:
        fixed["a_noise_mute"] = 0.0
    names = PARAMS + (NOISE if noise else [])
    synth = Surge()
    patch = os.path.join(TEMPLATES, template + ".fxp")
    synth.load(patch, fixed)
    prm = [synth.plugin.parameters[n] for n in names]

    def terms(x):
        for p, v in zip(prm, x[:-1]):
            p.raw_value = float(np.clip(v, 0, 1))
        sh = (x[-1] - 0.5) * 2 * SHIFT_MS / 1000
        m = synth.render_mono(events(shifted(lst, sh), t0, t1), t1 - t0, len(ref))
        if m is None:
            return None
        s, e = dist(m, ref)
        h = harmonics(m, ons, f0s) - h_ref
        return [s, e, float(np.mean(np.abs(h - h.mean(0)))), float(np.mean(np.abs(note_env(m, ons) - ne_ref)))]

    def loss(x):
        t = terms(x)
        return 99.0 if t is None else 0.5 * t[0] + sum(t[1:])

    x0 = np.clip([p.raw_value for p in prm] + [0.5], 0.05, 0.95)
    es = cma.CMAEvolutionStrategy(x0, 0.25, dict(bounds=[0, 1], seed=1, maxiter=iters, popsize=12, verbose=-9))
    while not es.stop():
        xs = es.ask()
        es.tell(xs, [loss(x) for x in xs])
    x = es.result.xbest
    return dict(
        part=part,
        patch=patch,
        filter=filt,
        loss=round(float(es.result.fbest), 3),
        spec_env_harmonics_noteenv=[round(v, 3) for v in terms(x)],
        shift_ms=round(float((x[-1] - 0.5) * 2 * SHIFT_MS), 2),
        params={**fixed, **{n: float(np.clip(v, 0, 1)) for n, v in zip(names, x[:-1])}},
    )


def main(song, args):
    lst = read_notes(args.midi) if args.midi else None
    print(json.dumps(fit(song, args.part, args.template, args.filter, args.bars, args.iters, args.noise, lst), ensure_ascii=False, indent=1))


def main_render(song, args):
    pick = json.loads(open(args.params).read())
    lst = read_notes(args.midi) if args.midi else song.part_notes(args.part)
    sh = pick.get("shift_ms", 0.0) / 1000
    synth = Surge()
    synth.load(pick["patch"], pick["params"])
    n = song.frames()
    y = synth.render(events(shifted(lst, sh)), n / SR)[:n]
    sf.write(args.out, y, SR, subtype="FLOAT")
    print(json.dumps(dict(part=args.part, notes=len(lst), out=args.out)))
