import argparse
import csv
import json
import re
import tomllib
from pathlib import Path

PPQ = 480
SIXTEENTH = PPQ // 4
HOOK = ["も", "う", "い", "っ", "か", "い"]
SMALL = set("ゃゅょぁぃぅぇぉゎ")
NOTE = re.compile(r"（[^）]*）")


def morae(reading):
    out = []
    for c in reading:
        if c in SMALL and out:
            out[-1] += c
        else:
            out.append(c)
    return out


def read_lyrics(path):
    lines = {}
    section = None
    index = 0
    for raw in Path(path).read_text(encoding="utf-8").splitlines():
        if raw.startswith("#"):
            section = raw[1:].split("|")[0]
            index = 0
            continue
        text, reading = raw.split("|")
        if not reading:
            continue
        index += 1
        note = NOTE.search(text)
        lines[(section, index)] = (NOTE.sub("", text).strip(), reading, note.group(0)[1:-1] if note else "")
    return lines


def words_of(text, reading, sung):
    chunks = text.split()
    readings = reading.split()
    counts = [len(morae(r)) for r in readings]
    if len(chunks) != len(readings) or sum(counts) != len(sung):
        return [{"text": text, "morae": list(range(len(sung)))}]
    out = []
    start = 0
    for chunk, count in zip(chunks, counts):
        out.append({"text": chunk, "morae": list(range(start, start + count))})
        start += count
    return out


def mark_hooks(rows):
    hook = 0
    i = 0
    while i < len(rows):
        if [r["mora"] for r in rows[i : i + len(HOOK)]] == HOOK:
            hook += 1
            for r in rows[i : i + len(HOOK)]:
                r["hook"] = hook
            i += len(HOOK)
        else:
            i += 1
    return hook


def build(score_path, vocal_path):
    score = tomllib.loads(Path(score_path).read_text(encoding="utf-8"))
    lyrics = read_lyrics(Path(score_path).parent / score["lyrics"])
    with open(vocal_path, encoding="utf-8") as f:
        rows = list(csv.DictReader(f, delimiter="\t"))
    for r in rows:
        tick = int(r["tick"])
        ticks = int(r["ticks"])
        if tick % SIXTEENTH or ticks % SIXTEENTH:
            raise SystemExit(f"16分の格子から外れたノート: {r}")
        r["start"] = tick // SIXTEENTH
        r["length"] = ticks // SIXTEENTH
    hooks = mark_hooks(rows)

    sections = []
    bar = 0
    for s in score["sections"]:
        sections.append({"name": s["name"], "startBar": bar, "bars": s["bars"]})
        bar += s["bars"]

    grouped = {}
    for r in rows:
        grouped.setdefault((r["section"], int(r["line"])), []).append(r)
    lines = []
    for (section, index), sung in grouped.items():
        text, reading, note = lyrics[(section, index)]
        lines.append(
            {
                "section": section,
                "index": index,
                "text": text,
                "note": note,
                "words": words_of(text, reading, sung),
                "morae": [
                    {
                        "text": r["mora"],
                        "kind": r["kind"],
                        "start": r["start"],
                        "length": r["length"],
                        "hook": r.get("hook", 0),
                    }
                    for r in sung
                ],
            }
        )
    return {
        "bpm": score["bpm"],
        "beatsPerBar": 4,
        "bars": bar,
        "hooks": hooks,
        "sections": sections,
        "lines": lines,
    }


def main():
    parser = argparse.ArgumentParser(
        description="vocal.tsv と score.toml から、16分音符単位の歌詞タイミングJSONを作る"
    )
    parser.add_argument("score")
    parser.add_argument("vocal")
    parser.add_argument("out")
    args = parser.parse_args()
    song = build(args.score, args.vocal)
    if song["bars"] != sum(s["bars"] for s in song["sections"]):
        raise SystemExit("小節数が合わない")
    Path(args.out).parent.mkdir(parents=True, exist_ok=True)
    Path(args.out).write_text(json.dumps(song, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
    print(f"{len(song['lines'])}行、フック{song['hooks']}回、{song['bars']}小節 → {args.out}")


if __name__ == "__main__":
    main()
