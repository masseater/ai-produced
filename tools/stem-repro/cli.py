import argparse

import design
import interstem
import kick
import mastering
import noise_resynth
import note_shape
import notes_filter
import optimize
import place
import search
import shape
import stems
import timbre
from song import Song


def parser():
    p = argparse.ArgumentParser(prog="stem-repro")
    p.add_argument("config", help="song.json")
    sub = p.add_subparsers(dest="cmd", required=True)
    sub.add_parser("stems").set_defaults(run=stems.main)

    s = sub.add_parser("interstem")
    s.add_argument("--section", required=True)
    s.add_argument("--kick", required=True)
    s.add_argument("--bass", required=True)
    s.add_argument("--repro")
    s.set_defaults(run=interstem.main)

    s = sub.add_parser("mastering")
    s.add_argument("--bus", action="store_true")
    s.set_defaults(run=mastering.main)

    s = sub.add_parser("fit-bus-master")
    s.add_argument("--stems-dir", required=True)
    s.add_argument("--out", required=True)
    s.set_defaults(run=mastering.main_fit)

    s = sub.add_parser("kick-fit")
    s.add_argument("--stem", required=True)
    s.add_argument("--section", required=True)
    s.add_argument("--length", type=float, default=0.42)
    s.add_argument("--hits", type=int, default=16)
    s.add_argument("--maxiter", type=int, default=60)
    s.add_argument("--seed", type=int, default=3)
    s.set_defaults(run=kick.main)

    s = sub.add_parser("kick-track")
    s.add_argument("--stem", required=True)
    s.add_argument("--section", required=True)
    s.add_argument("--params", required=True)
    s.add_argument("--length", type=float, default=0.42)
    s.add_argument("--hits", type=int, default=16)
    s.add_argument("--out", required=True)
    s.set_defaults(run=kick.main_track)

    s = sub.add_parser("search")
    s.add_argument("--out", required=True)
    s.add_argument("--workers", type=int, default=4)
    s.add_argument("--top", type=int, default=50)
    s.set_defaults(run=search.main)

    s = sub.add_parser("optimize")
    s.add_argument("--part", required=True)
    s.add_argument("--patch", required=True)
    s.add_argument("--maxiter", type=int, default=25)
    s.add_argument("--popsize", type=int, default=10)
    s.set_defaults(run=optimize.main)

    s = sub.add_parser("design")
    s.add_argument("--part", required=True)
    s.add_argument("--template", default="Init Saw")
    s.add_argument("--filter", choices=list(design.FILTERS), default="lp24")
    s.add_argument("--bars", type=float, nargs=2, required=True)
    s.add_argument("--iters", type=int, default=150)
    s.add_argument("--noise", action="store_true")
    s.add_argument("--midi")
    s.set_defaults(run=design.main)

    s = sub.add_parser("render")
    s.add_argument("--part", required=True)
    s.add_argument("--params", required=True)
    s.add_argument("--midi")
    s.add_argument("--out", required=True)
    s.set_defaults(run=design.main_render)

    s = sub.add_parser("filter-notes")
    s.add_argument("--stem", required=True)
    s.add_argument("--midi", required=True)
    s.add_argument("--bars", type=float, nargs=2)
    s.add_argument("--threshold", type=float, default=-30)
    s.set_defaults(run=notes_filter.main)

    s = sub.add_parser("edit-notes")
    s.add_argument("--midi", required=True)
    s.add_argument("--track", type=int)
    s.add_argument("--out", required=True)
    s.add_argument("--splice")
    s.add_argument("--splice-bars", type=float, nargs="+", default=[])
    s.add_argument("--grid", type=int)
    s.add_argument("--fixed", action="store_true")
    s.set_defaults(run=notes_filter.main_edit)

    s = sub.add_parser("place")
    s.add_argument("--stem", required=True)
    s.add_argument("--raw", required=True)
    s.add_argument("--out", required=True)
    s.set_defaults(run=place.main)

    s = sub.add_parser("shape")
    s.add_argument("--stem", required=True)
    s.add_argument("--raw", required=True)
    s.add_argument("--out", required=True)
    s.add_argument("--bars-only", action="store_true")
    s.set_defaults(run=shape.main)

    s = sub.add_parser("note-shape")
    s.add_argument("--stem", required=True)
    s.add_argument("--raw", required=True)
    s.add_argument("--out", required=True)
    s.add_argument("--part")
    s.add_argument("--midi")
    s.add_argument("--shift-ms", type=float, default=0.0)
    s.set_defaults(run=note_shape.main)

    s = sub.add_parser("timbre")
    s.add_argument("--stem", required=True)
    s.add_argument("--repro", required=True)
    s.add_argument("--bars", type=float, nargs=2, required=True)
    s.add_argument("--part")
    s.add_argument("--midi")
    s.set_defaults(run=timbre.main)

    s = sub.add_parser("resynth")
    s.add_argument("--stem", required=True)
    s.add_argument("--fft", type=int, default=512)
    s.add_argument("--out", required=True)
    s.set_defaults(run=noise_resynth.main)
    return p


if __name__ == "__main__":
    a = parser().parse_args()
    a.run(Song(a.config), a)
