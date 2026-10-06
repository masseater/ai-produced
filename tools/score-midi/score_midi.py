import argparse
import csv
import re
import sys
import tomllib
from dataclasses import dataclass, field
from pathlib import Path

import mido

TPQ = 480
SIXTEENTH = TPQ // 4
BAR = TPQ * 4

NOTE_PC = {"C": 0, "D": 2, "E": 4, "F": 5, "G": 7, "A": 9, "B": 11}
CHORD_SHAPES = {
    "": (0, 4, 7),
    "m": (0, 3, 7),
    "sus4": (0, 5, 7),
    "m7": (0, 3, 7, 10),
    "maj7": (0, 4, 7, 11),
    "7": (0, 4, 7, 10),
    "add9": (0, 4, 7, 14),
}
PAD_COLOR = {"": "add9", "m": "m7", "sus4": "sus4"}

SMALL_KANA = set("ゃゅょぁぃぅぇぉゎ")
VOWEL_OF = {}
for kana, vowel in [
    ("あかさたなはまやらわがざだばぱぁゃゎ", "a"),
    ("いきしちにひみりぎじぢびぴぃ", "i"),
    ("うくすつぬふむゆるぐずづぶぷぅゅ", "u"),
    ("えけせてねへめれげぜでべぺぇ", "e"),
    ("おこそとのほもよろをごぞどぼぽぉょ", "o"),
]:
    for char in kana:
        VOWEL_OF[char] = vowel

TRACKS = [
    ("Vocal", 0, 54),
    ("Drums", 9, 0),
    ("Bass", 1, 38),
    ("Pad", 2, 89),
    ("Saw", 3, 81),
    ("Arp", 4, 80),
    ("Piano", 5, 0),
    ("Lead", 6, 81),
]

KICK, RIM, SNARE, CLAP, HAT, OPEN_HAT, CRASH, TOM_LOW, TOM_MID, TOM_HIGH = (
    36, 37, 38, 39, 42, 46, 49, 45, 47, 50,
)


@dataclass
class Note:
    track: str
    start: int
    length: int
    pitch: int
    velocity: int
    lyric: str = ""


@dataclass
class Syllable:
    section: str
    line: int
    mora: str
    vowel: str
    kind: str
    start: int
    length: int
    pitch: int
    mode: str
    text: str


@dataclass
class Song:
    notes: list = field(default_factory=list)
    syllables: list = field(default_factory=list)
    chords: list = field(default_factory=list)
    markers: list = field(default_factory=list)
    silences: list = field(default_factory=list)
    problems: list = field(default_factory=list)


def pitch_of(name):
    match = re.fullmatch(r"([A-G])(#|b)?(-?\d)", name)
    if not match:
        raise ValueError(f"bad pitch {name}")
    letter, accidental, octave = match.groups()
    shift = {"#": 1, "b": -1}.get(accidental, 0)
    return NOTE_PC[letter] + shift + (int(octave) + 1) * 12


def name_of(pitch):
    names = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"]
    return f"{names[pitch % 12]}{pitch // 12 - 1}"


def parse_chord(symbol):
    match = re.fullmatch(r"([A-G])(#|b)?(.*)", symbol)
    letter, accidental, quality = match.groups()
    root = (NOTE_PC[letter] + {"#": 1, "b": -1}.get(accidental, 0)) % 12
    if quality not in CHORD_SHAPES:
        raise ValueError(f"bad chord {symbol}")
    return root, quality


def moras(reading):
    out = []
    for char in re.sub(r"\s", "", reading):
        if char in SMALL_KANA and out:
            out[-1] += char
        else:
            out.append(char)
    return out


def vowel(mora):
    if mora == "っ":
        return "Q"
    if mora == "ん":
        return "N"
    return VOWEL_OF.get(mora[-1], "?")


def read_lyrics(path):
    sections = {}
    current = None
    with open(path, encoding="utf-8") as handle:
        for raw in handle:
            line = raw.rstrip("\n")
            if not line:
                continue
            if line.startswith("#"):
                current = line[1:].split("|")[0]
                sections[current] = []
                continue
            text, reading = line.split("|")
            if reading.strip():
                sections[current].append((text, reading))
    return sections


def expand(tokens, motifs):
    return re.sub(r"\$(\w+)", lambda m: motifs[m.group(1)], tokens).split()


def voice(pcs, previous, low=52, high=76):
    best = None
    for rotation in range(len(pcs)):
        order = pcs[rotation:] + pcs[:rotation]
        for base in range(low, low + 12):
            if base % 12 != order[0]:
                continue
            chord = [base]
            for pc in order[1:]:
                nxt = chord[-1] + 1
                while nxt % 12 != pc:
                    nxt += 1
                chord.append(nxt)
            if chord[-1] > high:
                continue
            cost = (
                sum(abs(a - b) for a, b in zip(sorted(chord), sorted(previous)))
                if previous
                else abs(sum(chord) / len(chord) - 64)
            )
            if best is None or cost < best[0]:
                best = (cost, chord)
    return best[1]


def bass_pitch(root):
    return 28 + (root - 4) % 12


def place_vocal(song, section, start_bar, entries, lyric_lines, motifs):
    if len(entries) != len(lyric_lines):
        song.problems.append(
            f"{section['name']}: {len(entries)} melody lines for {len(lyric_lines)} lyric lines"
        )
        return
    for index, (entry, (text, reading)) in enumerate(zip(entries, lyric_lines), 1):
        tokens = expand(entry["notes"], motifs)
        sung = [t for t in tokens if not t.startswith("r")]
        line_moras = moras(reading)
        if len(sung) != len(line_moras):
            song.problems.append(
                f"{section['name']} line {index}: {len(sung)} notes for {len(line_moras)} morae ({reading})"
            )
            continue
        tick = (start_bar + entry["at"]) * BAR
        mode = entry.get("mode", section.get("mode", "Standard"))
        mora_iter = iter(line_moras)
        for token in tokens:
            if token.startswith("r"):
                tick += int(token[1:]) * SIXTEENTH
                continue
            length_text, pitch_text = token.split(":")
            length = int(length_text) * SIXTEENTH
            mora = next(mora_iter)
            if pitch_text == "cl":
                if mora != "っ":
                    song.problems.append(f"{section['name']} line {index}: closure on {mora}")
                song.syllables.append(
                    Syllable(section["name"], index, mora, "Q", "closure", tick, length, 0, mode, text)
                )
            else:
                if mora == "っ":
                    song.problems.append(f"{section['name']} line {index}: っ needs cl")
                pitch = pitch_of(pitch_text)
                song.syllables.append(
                    Syllable(section["name"], index, mora, vowel(mora), "note", tick, length, pitch, mode, text)
                )
                song.notes.append(Note("Vocal", tick, length, pitch, 96, mora))
            tick += length


def style_at(parts, key, index):
    spec = parts.get(key, "none").split()
    expanded = []
    for item in spec:
        name, _, count = item.partition("*")
        expanded.extend([name] * int(count or 1))
    return expanded[index] if len(spec) > 1 else expanded[0]


def hit(song, bar_tick, pos, pitch, velocity, length=1):
    song.notes.append(Note("Drums", bar_tick + int(pos * SIXTEENTH), int(length * SIXTEENTH), pitch, velocity))


def drums(song, style, bar_tick, bar_index, bars, energy):
    v = lambda base: max(1, min(127, int(base * (0.55 + 0.45 * energy))))
    phrase_end = bar_index % 4 == 3
    if style == "none":
        return
    if style == "halftime_light":
        hit(song, bar_tick, 8, RIM, v(70))
        for pos in range(0, 16, 2):
            hit(song, bar_tick, pos, HAT, v(45 if pos % 4 else 60))
        return
    if style == "halftime":
        for pos in (0, 10):
            hit(song, bar_tick, pos, KICK, v(100))
        hit(song, bar_tick, 8, SNARE, v(95))
        for pos in range(0, 16, 2):
            hit(song, bar_tick, pos, HAT, v(50 if pos % 4 else 70))
        if bar_index == 0:
            hit(song, bar_tick, 0, CRASH, v(80), 8)
        return
    if style == "heartbeat":
        hit(song, bar_tick, 0, KICK, v(70))
        hit(song, bar_tick, 3, KICK, v(55))
        return
    if style == "build":
        for pos in (0, 4, 8, 12):
            hit(song, bar_tick, pos, KICK, v(105))
        roll_bar = bar_index - (bars - 4)
        if roll_bar < 0:
            for pos in (4, 12):
                hit(song, bar_tick, pos, SNARE, v(100))
            for pos in range(2, 16, 4):
                hit(song, bar_tick, pos, OPEN_HAT, v(60))
        else:
            step = {0: 4, 1: 2, 2: 2, 3: 1}[roll_bar]
            for pos in range(0, 16, step):
                ramp = (roll_bar * 16 + pos) / 64
                hit(song, bar_tick, pos, SNARE, v(60 + 60 * ramp))
        if bar_index == 0:
            hit(song, bar_tick, 0, CRASH, v(90), 8)
        return
    if style in ("chorus", "last"):
        for pos in (0, 6, 8):
            hit(song, bar_tick, pos, KICK, v(115))
        for pos in (4, 12):
            hit(song, bar_tick, pos, SNARE, v(115))
            hit(song, bar_tick, pos, CLAP, v(85))
        for pos in range(0, 16, 2):
            hit(song, bar_tick, pos, OPEN_HAT if pos == 14 else HAT, v(70 if pos % 4 else 90))
        crash_every = 2 if style == "last" else 4
        if bar_index % crash_every == 0:
            hit(song, bar_tick, 0, CRASH, v(110), 8)
        if phrase_end:
            for pos, tom in zip(range(12, 16), (TOM_HIGH, TOM_HIGH, TOM_MID, TOM_LOW)):
                hit(song, bar_tick, pos, tom, v(105))
        return
    if style == "four":
        for pos in (0, 4, 8, 12):
            hit(song, bar_tick, pos, KICK, v(120))
        for pos in (4, 12):
            hit(song, bar_tick, pos, CLAP, v(110))
        for pos in range(16):
            hit(song, bar_tick, pos, OPEN_HAT if pos % 4 == 2 else HAT, v(80 if pos % 4 == 2 else 50))
        if bar_index == 0:
            hit(song, bar_tick, 0, CRASH, v(110), 8)
        return
    if style == "accel":
        stage = bar_index // 2
        if bar_index >= bars - 1:
            return
        if stage == 0:
            for pos in (0, 4, 8, 12):
                hit(song, bar_tick, pos, KICK, v(100))
        elif stage == 1:
            for pos in range(0, 16, 2):
                hit(song, bar_tick, pos, KICK, v(105))
                hit(song, bar_tick, pos, SNARE, v(80 + 2 * pos))
        elif stage == 2:
            for pos in range(0, 16, 1):
                hit(song, bar_tick, pos, SNARE, v(85 + 2 * pos))
            for pos in (0, 4, 8, 12):
                hit(song, bar_tick, pos, KICK, v(110))
        else:
            for half in range(32):
                hit(song, bar_tick, half / 2, SNARE, v(90 + half), 0.5)
            for pos in (0, 4, 8, 12):
                hit(song, bar_tick, pos, KICK, v(115))
        return
    if style == "hit":
        hit(song, bar_tick, 0, KICK, v(127))
        hit(song, bar_tick, 0, CRASH, v(120), 16)
        return
    if style == "outro":
        if bar_index < 4:
            hit(song, bar_tick, 0, KICK, v(80))
            hit(song, bar_tick, 8, RIM, v(60))
        return
    raise ValueError(f"unknown drum style {style}")


def bass(song, style, bar_tick, root, energy, next_root):
    pitch = bass_pitch(root)
    v = lambda base: max(1, min(127, int(base * (0.55 + 0.45 * energy))))
    add = lambda pos, length, p=pitch, vel=100: song.notes.append(
        Note("Bass", bar_tick + pos * SIXTEENTH, length * SIXTEENTH, p, v(vel))
    )
    if style == "none":
        return
    if style == "sustain":
        add(0, 16)
    elif style == "half":
        add(0, 8)
        add(8, 8)
    elif style == "eighths":
        for pos in range(0, 16, 2):
            add(pos, 2, pitch + 12 if pos in (6, 14) else pitch, 110 if pos % 4 == 0 else 90)
    elif style == "offbeat":
        add(0, 2, pitch, 115)
        for pos in (2, 6, 10, 14):
            add(pos, 2, pitch, 105)
        add(7, 1, pitch + 12, 80)
        add(15, 1, pitch + 12, 80)
    elif style == "build":
        for pos in range(0, 16, 2):
            add(pos, 2, pitch, 90 + pos * 2)
    elif style == "walk":
        add(0, 6)
        add(6, 6)
        target = bass_pitch(next_root)
        step = 1 if target > pitch else -1
        add(12, 4, pitch + step * 2 if abs(target - pitch) > 2 else pitch)
    elif style == "stop":
        add(0, 4, pitch, 120)
    else:
        raise ValueError(f"unknown bass style {style}")


def comp(song, part, style, bar_tick, voicing, energy):
    v = lambda base: max(1, min(127, int(base * (0.55 + 0.45 * energy))))
    add = lambda pos, length, p, vel: song.notes.append(
        Note(part, bar_tick + pos * SIXTEENTH, length * SIXTEENTH, p, v(vel))
    )
    if style == "none":
        return
    if style == "sustain":
        for p in voicing:
            add(0, 16, p, 70)
    elif style == "stabs":
        for pos in (0, 3, 6, 8, 11, 14):
            for p in voicing:
                add(pos, 2, p + 12, 65)
    elif style == "offstabs":
        for pos in (2, 6, 10, 14):
            for p in voicing:
                add(pos, 1, p + 12, 70)
    elif style == "broken":
        order = [voicing[0], voicing[2], voicing[1], voicing[-1]] * 2
        for i, p in enumerate(order):
            add(i * 2, 2, p, 70 if i % 2 else 80)
    elif style == "quarters":
        for i, p in enumerate([voicing[0], voicing[1], voicing[2], voicing[-1]]):
            add(i * 4, 4, p, 60)
    elif style == "hit":
        for p in voicing:
            add(0, 4, p + 12, 110)
    elif style == "arp":
        cycle = sorted(set(p + 12 for p in voicing))
        pattern = cycle + cycle[-2:0:-1]
        for pos in range(16):
            add(pos, 1, pattern[pos % len(pattern)], 55 if pos % 4 else 70)
    else:
        raise ValueError(f"unknown {part} style {style}")


def build(score, base_dir):
    song = Song()
    lyrics = read_lyrics(base_dir / score["lyrics"])
    motifs = score.get("motifs", {})
    bar = 0
    voicing = None
    all_chords = []
    for section in score["sections"]:
        symbols = section["chords"].split()
        if len(symbols) != section["bars"]:
            song.problems.append(f"{section['name']}: {len(symbols)} chords for {section['bars']} bars")
        all_chords.extend((section, i, s) for i, s in enumerate(symbols))
    for position, (section, index, symbol) in enumerate(all_chords):
        if index == 0:
            song.markers.append((position * BAR, section["name"]))
            song.silences.extend(
                (
                    (position + int(b)) * BAR + int(lo) * SIXTEENTH,
                    (position + int(b)) * BAR + int(hi) * SIXTEENTH,
                )
                for b, lo, hi in (re.fullmatch(r"(\d+):(\d+)-(\d+)", s).groups() for s in section.get("silent", []))
            )
            place_vocal(song, section, position, section.get("vocal", []), lyrics.get(section["name"], []), motifs)
        bar_tick = position * BAR
        parts = section.get("parts", {})
        energy = section.get("energy", 0.5)
        song.chords.append((position + 1, section["name"], symbol))
        if symbol == "NC":
            continue
        root, quality = parse_chord(symbol)
        pad_quality = PAD_COLOR.get(quality, quality)
        pcs = [(root + i) % 12 for i in CHORD_SHAPES[pad_quality]]
        voicing = voice(pcs, voicing)
        following = all_chords[position + 1][2] if position + 1 < len(all_chords) else symbol
        next_root = parse_chord(following)[0] if following != "NC" else root
        drums(song, style_at(parts, "drums", index), bar_tick, index, section["bars"], energy)
        bass(song, style_at(parts, "bass", index), bar_tick, root, energy, next_root)
        for part in ("Pad", "Saw", "Arp", "Piano"):
            comp(song, part, style_at(parts, part.lower(), index), bar_tick, voicing, energy)
        if style_at(parts, "lead", index) == "hook":
            for token_start, length, pitch in hook_shape(motifs["HOOK"]):
                song.notes.append(Note("Lead", bar_tick + token_start, length, pitch + 12, 70))
        bar = position + 1
    double_vocal(song, score)
    apply_silence(song)
    check(song, score, bar)
    return song


def hook_shape(tokens):
    tick = 0
    out = []
    for token in tokens.split():
        if token.startswith("r"):
            tick += int(token[1:]) * SIXTEENTH
            continue
        length_text, pitch_text = token.split(":")
        length = int(length_text) * SIXTEENTH
        if pitch_text != "cl":
            out.append((tick, length, pitch_of(pitch_text)))
        tick += length
    return out


def double_vocal(song, score):
    sections = {s["name"]: s for s in score["sections"]}
    for syllable in song.syllables:
        if syllable.kind == "note" and sections[syllable.section].get("parts", {}).get("lead") == "double":
            song.notes.append(Note("Lead", syllable.start, syllable.length, syllable.pitch + 12, 65))


def apply_silence(song):
    kept = []
    for note in song.notes:
        end = note.start + note.length
        for lo, hi in song.silences:
            if lo <= note.start < hi:
                break
            if note.start < lo < end:
                note.length = lo - note.start
        else:
            kept.append(note)
    song.notes = kept


def check(song, score, bars):
    rules = score.get("rules", {})
    if bars != score["bars"]:
        song.problems.append(f"song has {bars} bars, expected {score['bars']}")
    low, high = pitch_of(rules["range"][0]), pitch_of(rules["range"][1])
    peak = pitch_of(rules["peak"])
    over_peak = [s for s in song.syllables if s.kind == "note" and s.pitch > peak]
    if len(over_peak) > rules.get("peak_exceptions", 0):
        song.problems.append(f"{len(over_peak)} notes above {rules['peak']}")
    for s in song.syllables:
        if s.kind == "note" and not low <= s.pitch <= high:
            song.problems.append(f"{s.section} line {s.line}: {name_of(s.pitch)} out of range")
    long_ticks = rules["long_sixteenths"] * SIXTEENTH
    for previous, s in zip([None] + song.syllables, song.syllables):
        hook_tail = previous is not None and previous.mora == "か" and s.mora == "い"
        if s.kind == "note" and s.length >= long_ticks and s.vowel not in "aoe" and not hook_tail:
            song.problems.append(f"{s.section} line {s.line}: long note on {s.mora}")
    for a, b in zip(song.syllables, song.syllables[1:]):
        if a.start + a.length > b.start:
            song.problems.append(f"{a.section} line {a.line}: overlap at {a.mora}")
        if (a.section, a.line) != (b.section, b.line) and a.start + a.length == b.start:
            song.problems.append(f"{b.section} line {b.line}: no breath before line")


def write_midi(song, score, path, start=0, end=None):
    midi = mido.MidiFile(ticks_per_beat=TPQ, charset="utf-8")
    conductor = mido.MidiTrack()
    conductor.append(mido.MetaMessage("set_tempo", tempo=mido.bpm2tempo(score["bpm"]), time=0))
    conductor.append(mido.MetaMessage("time_signature", numerator=4, denominator=4, time=0))
    conductor.append(mido.MetaMessage("key_signature", key=score["key"], time=0))
    events = [(t - start, mido.MetaMessage("marker", text=name)) for t, name in song.markers if start <= t < (end or 1 << 30)]
    append_events(conductor, events)
    midi.tracks.append(conductor)
    for name, channel, program in TRACKS:
        track = mido.MidiTrack()
        track.append(mido.MetaMessage("track_name", name=name, time=0))
        if channel != 9:
            track.append(mido.Message("program_change", channel=channel, program=program, time=0))
        events = []
        for note in song.notes:
            if note.track != name or note.start < start or (end is not None and note.start >= end):
                continue
            at = note.start - start
            if note.lyric:
                events.append((at, mido.MetaMessage("lyrics", text=note.lyric)))
            events.append((at, mido.Message("note_on", channel=channel, note=note.pitch, velocity=note.velocity)))
            events.append((at + max(1, note.length - 10), mido.Message("note_off", channel=channel, note=note.pitch, velocity=0)))
        append_events(track, events)
        midi.tracks.append(track)
    midi.save(path)


def append_events(track, events):
    order = {"note_off": 0, "lyrics": 1, "note_on": 2}
    events.sort(key=lambda e: (e[0], order.get(e[1].type, 1)))
    now = 0
    for at, message in events:
        track.append(message.copy(time=at - now))
        now = at
    track.append(mido.MetaMessage("end_of_track", time=0))


def write_tables(song, out_dir):
    with open(out_dir / "vocal.tsv", "w", encoding="utf-8", newline="") as handle:
        writer = csv.writer(handle, delimiter="\t", lineterminator="\n")
        writer.writerow(["section", "line", "text", "mora", "vowel", "kind", "bar", "beat", "tick", "ticks", "note", "midi", "mode"])
        for s in song.syllables:
            writer.writerow([
                s.section, s.line, s.text, s.mora, s.vowel, s.kind,
                s.start // BAR + 1, round(s.start % BAR / TPQ + 1, 3), s.start, s.length,
                name_of(s.pitch) if s.kind == "note" else "", s.pitch if s.kind == "note" else "", s.mode,
            ])
    with open(out_dir / "chords.tsv", "w", encoding="utf-8", newline="") as handle:
        writer = csv.writer(handle, delimiter="\t", lineterminator="\n")
        writer.writerow(["bar", "section", "chord"])
        writer.writerows(song.chords)


def main():
    parser = argparse.ArgumentParser(description="Build MIDI and a vocal syllable table from a score TOML")
    parser.add_argument("score", type=Path)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    with open(args.score, "rb") as handle:
        score = tomllib.load(handle)
    song = build(score, args.score.parent)
    if song.problems:
        print("\n".join(song.problems), file=sys.stderr)
        sys.exit(1)
    sections_dir = args.out / "sections"
    sections_dir.mkdir(parents=True, exist_ok=True)
    write_midi(song, score, args.out / "full.mid")
    bounds = song.markers + [(score["bars"] * BAR, "")]
    for number, ((start, name), (end, _)) in enumerate(zip(bounds, bounds[1:]), 1):
        write_midi(song, score, sections_dir / f"{number:02d}-{name}.mid", start, end)
    write_tables(song, args.out)
    notes = [s for s in song.syllables if s.kind == "note"]
    print(f"bars={score['bars']} vocal_notes={len(notes)} range={name_of(min(s.pitch for s in notes))}-{name_of(max(s.pitch for s in notes))}")


if __name__ == "__main__":
    main()
