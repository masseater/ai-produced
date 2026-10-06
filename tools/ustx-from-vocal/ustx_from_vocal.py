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
DEFAULT_MODE = "Standard"
SUNG_AS = {"を": "お"}
PORTAMENTO = 40
BREATH_LYRIC = "AP"
CURVES = ("brec", "tenc", "voic", "velc", "dyn")
CURVE_DEFAULTS = {"brec": 0, "tenc": 0, "voic": 100, "velc": 100, "dyn": 0}
CURVE_RAMP = 120
NOTE_PC = {"C": 0, "D": 2, "E": 4, "F": 5, "G": 7, "A": 9, "B": 11}


def key_of(name):
    tonic = NOTE_PC[name[0]] + {"#": 1, "b": -1}.get(name[1:2], 0)
    return (tonic + 3) % 12 if name.endswith("m") else tonic % 12


def read_rows(path):
    with path.open(encoding="utf-8", newline="") as file:
        return list(csv.DictReader(file, delimiter="\t"))


def section_spans(score):
    spans, bar = {}, 0
    for section in score["sections"]:
        spans[section["name"]] = (bar * BAR, (bar + section["bars"]) * BAR)
        bar += section["bars"]
    return spans


def portamento(ms):
    return [{"x": -ms, "y": 0, "shape": "io"}, {"x": ms, "y": 0, "shape": "io"}]


def vibrato(length, settings):
    if not settings or length < settings["min_ticks"]:
        return {"length": 0, "period": 175, "depth": 25, "in": 10, "out": 10, "shift": 0, "drift": 0, "vol_link": 0}
    return {
        "length": settings["length"], "period": settings["period"], "depth": settings["depth"],
        "in": settings["fade_in"], "out": settings["fade_out"], "shift": 0, "drift": 0, "vol_link": 0,
    }


def build_notes(rows, colors, tuning):
    notes = []
    sections = tuning.get("section", {})
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
        style = sections.get(row["section"], {})
        mode = style.get("mode", row["mode"])
        if colors:
            color = "" if mode == DEFAULT_MODE and "" in colors else mode
            if color in colors:
                expressions.append({"index": 0, "abbr": "clr", "value": colors.index(color)})
            else:
                missing.add(mode)
        notes.append({
            "position": start,
            "duration": length,
            "tone": tone,
            "lyric": lyric,
            "pitch": {"data": portamento(style.get("portamento", PORTAMENTO)), "snap_first": True},
            "vibrato": vibrato(length, style.get("vibrato", True) and tuning.get("vibrato")),
            "tuning": 0,
            "phoneme_expressions": expressions,
            "phoneme_overrides": [],
        })
        notes[-1]["section"] = row["section"]
    if missing:
        sys.exit(f"modes not among --colors: {', '.join(sorted(missing))}")
    return notes


def shorten(notes, tuning):
    sections = tuning.get("section", {})
    for note, after in zip(notes, notes[1:] + [None]):
        ratio = sections.get(note["section"], {}).get("staccato")
        tied = after is not None and after["lyric"] == CLOSURE_LYRIC
        if ratio and note["lyric"] != CLOSURE_LYRIC and not tied:
            note["duration"] = max(60, round(note["duration"] * ratio / 30) * 30)


def add_breaths(notes, tuning):
    settings = tuning.get("breath")
    if not settings:
        return notes
    out = []
    for note in notes:
        end = out[-1]["position"] + out[-1]["duration"] if out else 0
        rest = note["position"] - end
        if out and rest >= settings["min_rest"] and note["section"] in settings["sections"]:
            length = min(settings["ticks"], rest - settings["gap"])
            out.append({**note, "position": note["position"] - length, "duration": length, "lyric": BREATH_LYRIC,
                        "vibrato": vibrato(0, None), "phoneme_expressions": list(note["phoneme_expressions"])})
        out.append(note)
    return out


def build_curves(spans, tuning):
    sections = tuning.get("section", {})
    curves = []
    for abbr in CURVES:
        values = [(start, end, sections.get(name, {}).get(abbr, CURVE_DEFAULTS[abbr])) for name, (start, end) in spans.items()]
        if all(value == CURVE_DEFAULTS[abbr] for _, _, value in values):
            continue
        xs, ys = [], []
        for start, end, value in values:
            xs += [start, end - CURVE_RAMP]
            ys += [value, value]
        curves.append({"xs": xs, "ys": ys, "abbr": abbr})
    return curves


def build_project(name, score, notes, singer, colors, phonemizer, curves):
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
            "notes": [{k: v for k, v in note.items() if k != "section"} for note in notes],
            "curves": curves,
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
    parser.add_argument("--tuning", type=Path, help="tuning TOML: per-section mode, curves (brec, tenc, voic, velc, dyn), portamento, vibrato, staccato; breath insertion")
    parser.add_argument("--colors", help="voice colors of the singer, comma separated, as `openutau-render singers` prints them; maps the mode column to clr")
    args = parser.parse_args()

    score = tomllib.loads(args.score.read_text(encoding="utf-8"))
    colors = args.colors.split(",") if args.colors else []
    tuning = tomllib.loads(args.tuning.read_text(encoding="utf-8")) if args.tuning else {}
    notes = build_notes(read_rows(args.vocal), colors, tuning)
    shorten(notes, tuning)
    notes = add_breaths(notes, tuning)
    curves = build_curves(section_spans(score), tuning)
    project = build_project(args.out.stem, score, notes, args.singer, colors, args.phonemizer, curves)
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(yaml.safe_dump(project, allow_unicode=True, sort_keys=False, width=1000), encoding="utf-8")
    closures = sum(1 for note in notes if note["lyric"] == CLOSURE_LYRIC)
    breaths = sum(1 for note in notes if note["lyric"] == BREATH_LYRIC)
    print(f"{args.out}: {len(notes)} notes ({closures} closures, {breaths} breaths), {project['voice_parts'][0]['duration'] // BAR} bars")


if __name__ == "__main__":
    main()
