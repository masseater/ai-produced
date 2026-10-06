import argparse
import csv
import sys
import tomllib
from pathlib import Path

import yaml

TPQ = 480
BAR = TPQ * 4
PHONEMIZER = "OpenUtau.Core.DiffSinger.DiffSingerJapanesePhonemizer"
CLOSURE_LYRIC = "っ"
SUNG_AS = {"を": "お"}
PORTAMENTO = [{"x": -40, "y": 0, "shape": "io"}, {"x": 40, "y": 0, "shape": "io"}]
NOTE_PC = {"C": 0, "D": 2, "E": 4, "F": 5, "G": 7, "A": 9, "B": 11}


def key_of(name):
    tonic = NOTE_PC[name[0]] + {"#": 1, "b": -1}.get(name[1:2], 0)
    return (tonic + 3) % 12 if name.endswith("m") else tonic % 12


def read_rows(path):
    with path.open(encoding="utf-8", newline="") as file:
        return list(csv.DictReader(file, delimiter="\t"))


def build_notes(rows, colors):
    notes = []
    missing = set()
    for row in rows:
        start, length = int(row["tick"]), int(row["ticks"])
        if row["kind"] == "closure":
            if not notes or notes[-1]["position"] + notes[-1]["duration"] != start:
                sys.exit(f"closure without a preceding note at tick {start}")
            tone, lyric = notes[-1]["tone"], CLOSURE_LYRIC
        else:
            tone, lyric = int(row["midi"]), SUNG_AS.get(row["mora"], row["mora"])
        if notes and notes[-1]["position"] + notes[-1]["duration"] > start:
            sys.exit(f"overlapping notes at tick {start}")
        expressions = []
        if colors:
            if row["mode"] in colors:
                expressions.append({"index": 0, "abbr": "clr", "value": colors.index(row["mode"])})
            else:
                missing.add(row["mode"])
        notes.append({
            "position": start,
            "duration": length,
            "tone": tone,
            "lyric": lyric,
            "pitch": {"data": [dict(point) for point in PORTAMENTO], "snap_first": True},
            "vibrato": {"length": 0, "period": 175, "depth": 25, "in": 10, "out": 10, "shift": 0, "drift": 0, "vol_link": 0},
            "tuning": 0,
            "phoneme_expressions": expressions,
            "phoneme_overrides": [],
        })
    if missing:
        sys.exit(f"modes not among --colors: {', '.join(sorted(missing))}")
    return notes


def build_project(name, score, notes, singer, colors, phonemizer):
    end = notes[-1]["position"] + notes[-1]["duration"]
    track = {
        "phonemizer": phonemizer,
        "renderer_settings": {},
        "track_name": "Vocal",
        "track_color": "Blue",
        "mute": False,
        "solo": False,
        "volume": 0,
        "pan": 0,
        "track_expressions": [],
        "voice_color_names": colors or [""],
    }
    if singer:
        track = {"singer": singer, **track}
    return {
        "name": name,
        "comment": "",
        "output_dir": "Vocal",
        "cache_dir": "UCache",
        "ustx_version": "0.10",
        "key": key_of(score["key"]),
        "time_signatures": [{"bar_position": 0, "beat_per_bar": 4, "beat_unit": 4}],
        "tempos": [{"position": 0, "bpm": score["bpm"]}],
        "tracks": [track],
        "voice_parts": [{
            "duration": -(-end // BAR) * BAR,
            "name": "Vocal",
            "comment": "",
            "track_no": 0,
            "position": 0,
            "notes": notes,
            "curves": [],
            "masked_curves": [],
        }],
        "wave_parts": [],
    }


def main():
    parser = argparse.ArgumentParser(
        description="Build an OpenUtau USTX project from vocal.tsv (tools/score-midi output). "
        "Closure rows become a note sung as っ, which the DiffSinger Japanese phonemizer turns into the cl phoneme.",
    )
    parser.add_argument("vocal", type=Path, help="vocal.tsv")
    parser.add_argument("--score", type=Path, required=True, help="score.toml, for bpm and key")
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--singer", help="singer id, as `openutau-render singers` prints it")
    parser.add_argument("--phonemizer", default=PHONEMIZER, help=f"phonemizer type name (default {PHONEMIZER})")
    parser.add_argument("--colors", help="voice colors of the singer, comma separated, as `openutau-render singers` prints them; maps the mode column to clr")
    args = parser.parse_args()

    score = tomllib.loads(args.score.read_text(encoding="utf-8"))
    colors = args.colors.split(",") if args.colors else []
    notes = build_notes(read_rows(args.vocal), colors)
    project = build_project(args.out.stem, score, notes, args.singer, colors, args.phonemizer)
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(yaml.safe_dump(project, allow_unicode=True, sort_keys=False, width=1000), encoding="utf-8")
    closures = sum(1 for note in notes if note["lyric"] == CLOSURE_LYRIC)
    print(f"{args.out}: {len(notes)} notes ({closures} closures), {project['voice_parts'][0]['duration'] // BAR} bars")


if __name__ == "__main__":
    main()
