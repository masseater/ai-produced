import json
import os
import signal
import subprocess
import sys
import time
from pathlib import Path

from audio import SR
from features import dist
from midi_notes import events
from surge import PATCHES, Surge


def excerpt(song, part, b0, b1):
    t0, t1 = b0 * song.bar, b1 * song.bar
    ev = events(song.notes_in(song.part_notes(part), b0, b1), t0, t1)
    ref = song.bars(song.stem(part), b0, b1).mean(1)
    return ev, ref, t1 - t0


def score(v):
    return v[0] + 0.5 * v[1]


def worker(config, stage, listfile, outfile, progfile):
    from song import Song

    song = Song(config)
    key = "search_bars" if stage == "1" else "rescore_bars"
    jobs = {p: excerpt(song, p, *song.parts[p][key]) for p in song.parts}
    rows = [json.loads(line) for line in Path(listfile).read_text().splitlines() if line]
    done = {json.loads(line)["f"] for line in open(outfile)} if os.path.exists(outfile) else set()
    synth = Surge()
    with open(progfile, "a") as prog, open(outfile, "a") as out:
        prog.write("READY\n")
        prog.flush()
        for f, parts in rows:
            if f in done:
                continue
            prog.write(f + "\n")
            prog.flush()
            synth.load(f)
            sc = {}
            for p in parts:
                ev, ref, dur = jobs[p]
                m = synth.render_mono(ev, dur, len(ref))
                sc[p] = [99.0, 99.0] if m is None else list(dist(m, ref))
            out.write(json.dumps(dict(f=f, sc=sc)) + "\n")
            out.flush()


def run_stage(config, stage, rows, wd, workers, timeout=30):
    wd = Path(wd)
    wd.mkdir(parents=True, exist_ok=True)
    for k in range(workers):
        (wd / f"list{k}").write_text("\n".join(json.dumps(r) for r in rows[k::workers]))

    def start(k):
        cmd = [sys.executable, __file__, "--worker", str(config), stage, str(wd / f"list{k}"), str(wd / f"out{k}.jsonl"), str(wd / f"prog{k}")]
        return subprocess.Popen(cmd, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, start_new_session=True)

    procs = {k: start(k) for k in range(workers)}
    bad = []
    while procs:
        time.sleep(2)
        for k, p in list(procs.items()):
            pf = wd / f"prog{k}"
            rc = p.poll()
            if rc is None:
                ready = pf.exists() and "READY" in pf.read_text().split("\n")
                if not ready or time.time() - pf.stat().st_mtime < timeout:
                    continue
                os.killpg(p.pid, signal.SIGKILL)
                p.wait()
                rc = -9
            if rc == 0:
                del procs[k]
                continue
            lines = [line for line in pf.read_text().split("\n") if line and line not in ("READY", "RESTART")]
            if lines:
                bad.append(lines[-1])
                with open(wd / f"out{k}.jsonl", "a") as out:
                    out.write(json.dumps(dict(f=lines[-1], sc={}, bad=True)) + "\n")
            with open(pf, "a") as prog:
                prog.write("RESTART\n")
            procs[k] = start(k)
    res = []
    for k in range(workers):
        f = wd / f"out{k}.jsonl"
        if f.exists():
            res += [json.loads(line) for line in f.read_text().splitlines()]
    return res, bad


def main(song, args):
    t = time.time()
    files = sorted(str(p) for p in Path(PATCHES).glob("patches_*/**/*.fxp"))
    parts = list(song.parts)
    r1, bad1 = run_stage(song.path, "1", [[f, parts] for f in files], Path(args.out) / "stage1", args.workers)
    print("stage1", len(r1), "patches", round(time.time() - t), "s, crashed", len(bad1), flush=True)
    need = {}
    for p in parts:
        ranked = sorted((r for r in r1 if p in r["sc"]), key=lambda r: score(r["sc"][p]))[:args.top]
        for r in ranked:
            need.setdefault(r["f"], []).append(p)
    r2, bad2 = run_stage(song.path, "2", [[f, ps] for f, ps in need.items()], Path(args.out) / "stage2", args.workers)
    print("stage2", len(r2), round(time.time() - t), "s", flush=True)
    final = {}
    for p in parts:
        ranked = sorted((dict(f=r["f"], spec=r["sc"][p][0], env=r["sc"][p][1]) for r in r2 if p in r["sc"]), key=lambda r: r["spec"] + 0.5 * r["env"])
        final[p] = ranked
        print(f"== {p}")
        for x in ranked[:5]:
            print(f"  {x['spec']:.2f} {x['env']:.2f} {os.path.relpath(x['f'], PATCHES)}")
    (Path(args.out) / "result.json").write_text(json.dumps(dict(final=final, crashed=bad1 + bad2), indent=0))


if __name__ == "__main__" and len(sys.argv) > 1 and sys.argv[1] == "--worker":
    worker(*sys.argv[2:7])
