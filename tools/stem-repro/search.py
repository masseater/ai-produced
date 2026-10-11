import json
import os
import time
from concurrent.futures import TimeoutError
from pathlib import Path

from pebble import ProcessExpired, ProcessPool

from features import dist
from midi_notes import events
from surge import PATCHES, Surge

STATE = {}


def excerpt(song, part, b0, b1):
    t0, t1 = b0 * song.bar, b1 * song.bar
    ev = events(song.notes_in(song.part_notes(part), b0, b1), t0, t1)
    ref = song.bars(song.stem(part), b0, b1).mean(1)
    return ev, ref, t1 - t0


def score(v):
    return v[0] + 0.5 * v[1]


def init(config, key):
    from song import Song

    song = Song(config)
    STATE["jobs"] = {p: excerpt(song, p, *song.parts[p][key]) for p in song.parts}
    STATE["synth"] = Surge()


def rate(f, parts):
    synth = STATE["synth"]
    synth.load(f)
    sc = {}
    for p in parts:
        ev, ref, dur = STATE["jobs"][p]
        m = synth.render_mono(ev, dur, len(ref))
        sc[p] = [99.0, 99.0] if m is None else list(dist(m, ref))
    return sc


def run_stage(config, key, rows, workers, timeout=30):
    res, bad = [], []
    with ProcessPool(max_workers=workers, max_tasks=200, initializer=init, initargs=(str(config), key)) as pool:
        futures = [(f, pool.schedule(rate, args=(f, parts), timeout=timeout)) for f, parts in rows]
        for f, fut in futures:
            try:
                res.append(dict(f=f, sc=fut.result()))
            except (TimeoutError, ProcessExpired):
                bad.append(f)
    return res, bad


def main(song, args):
    t = time.time()
    files = sorted(str(p) for p in Path(PATCHES).glob("patches_*/**/*.fxp"))
    parts = list(song.parts)
    r1, bad1 = run_stage(song.path, "search_bars", [(f, parts) for f in files], args.workers)
    print("stage1", len(r1), "patches", round(time.time() - t), "s, crashed", len(bad1), flush=True)
    need = {}
    for p in parts:
        for r in sorted((r for r in r1 if p in r["sc"]), key=lambda r: score(r["sc"][p]))[:args.top]:
            need.setdefault(r["f"], []).append(p)
    r2, bad2 = run_stage(song.path, "rescore_bars", list(need.items()), args.workers)
    print("stage2", len(r2), round(time.time() - t), "s", flush=True)
    final = {}
    for p in parts:
        ranked = sorted((dict(f=r["f"], spec=r["sc"][p][0], env=r["sc"][p][1]) for r in r2 if p in r["sc"]), key=lambda r: r["spec"] + 0.5 * r["env"])
        final[p] = ranked
        print(f"== {p}")
        for x in ranked[:5]:
            print(f"  {x['spec']:.2f} {x['env']:.2f} {os.path.relpath(x['f'], PATCHES)}")
    Path(args.out).mkdir(parents=True, exist_ok=True)
    (Path(args.out) / "result.json").write_text(json.dumps(dict(final=final, crashed=bad1 + bad2), indent=0))
