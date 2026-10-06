import argparse
import os
import uuid
from pathlib import Path

import mido
import rpp


def guid(seed):
    return "{" + str(uuid.uuid5(uuid.NAMESPACE_URL, seed)).upper() + "}"


def conductor(midi):
    tempo, numerator, denominator, markers = 500000, 4, 4, []
    tick = 0
    for message in midi.tracks[0]:
        tick += message.time
        if message.type == "set_tempo" and tick == 0:
            tempo = message.tempo
        elif message.type == "time_signature" and tick == 0:
            numerator, denominator = message.numerator, message.denominator
        elif message.type == "marker":
            markers.append((tick, message.text))
    return tempo, numerator, denominator, markers


def midi_events(track, length_ticks):
    events, last, tick = [], 0, 0
    for message in track:
        tick += message.time
        if message.is_meta or message.type == "sysex":
            continue
        data = message.bytes()
        events.append(["E", str(tick - last), *(f"{byte:02x}" for byte in data)])
        last = tick
    events.append(["E", str(max(0, length_ticks - last)), "b0", "7b", "00"])
    return events


def midi_track(name, track, ppq, length_ticks, length_seconds):
    source = rpp.Element("SOURCE", ["MIDI"], [["HASDATA", "1", str(ppq), "QN"], *midi_events(track, length_ticks)])
    item = rpp.Element("ITEM", [], [
        ["POSITION", "0"],
        ["LENGTH", f"{length_seconds:.6f}"],
        ["NAME", name],
        ["GUID", guid(f"item/{name}")],
        source,
    ])
    return rpp.Element("TRACK", [guid(f"track/{name}")], [["NAME", name], ["TRACKID", guid(f"track/{name}")], item])


def audio_track(name, path, length_seconds):
    item = rpp.Element("ITEM", [], [
        ["POSITION", "0"],
        ["LENGTH", f"{length_seconds:.6f}"],
        ["NAME", Path(path).name],
        ["GUID", guid(f"item/{name}")],
        rpp.Element("SOURCE", ["WAVE"], [["FILE", path]]),
    ])
    return rpp.Element("TRACK", [guid(f"track/{name}")], [["NAME", name], ["TRACKID", guid(f"track/{name}")], item])


def main():
    parser = argparse.ArgumentParser(
        description="Build a REAPER project from a multitrack MIDI file: tempo, section markers from MIDI marker events, "
        "one track per MIDI track with the notes embedded, and an audio track for the rendered vocal. No plugins are set.",
    )
    parser.add_argument("midi", type=Path)
    parser.add_argument("--out", type=Path, required=True, help="project .rpp")
    parser.add_argument("--audio", action="append", default=[], metavar="TRACK=WAV",
                        help="audio track in place of the MIDI track of the same name; WAV is relative to the project")
    parser.add_argument("--sample-rate", type=int, default=48000)
    args = parser.parse_args()

    midi = mido.MidiFile(args.midi)
    tempo, numerator, denominator, markers = conductor(midi)
    bpm = mido.tempo2bpm(tempo)
    ppq = midi.ticks_per_beat
    bar_ticks = ppq * numerator * 4 // denominator
    end_ticks = max(sum(message.time for message in track) for track in midi.tracks)
    length_ticks = -(-end_ticks // bar_ticks) * bar_ticks
    length_seconds = length_ticks / ppq * 60 / bpm
    audio = dict(entry.split("=", 1) for entry in args.audio)

    project = rpp.Element("REAPER_PROJECT", ["0.1", "7.0", "0"], [
        ["TEMPO", f"{bpm:g}", str(numerator), str(denominator)],
        ["SAMPLERATE", str(args.sample_rate), "0", "0"],
    ])
    for index, (tick, name) in enumerate(markers, start=1):
        project.append(["MARKER", str(index), f"{tick / ppq * 60 / bpm:.6f}", name, "0", "0", "1", "B", guid(f"marker/{index}")])
    for track in midi.tracks[1:]:
        if track.name in audio:
            project.append(audio_track(track.name, audio.pop(track.name), length_seconds))
        else:
            project.append(midi_track(track.name, track, ppq, length_ticks, length_seconds))
    for name, path in audio.items():
        project.append(audio_track(name, path, length_seconds))

    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(rpp.dumps(project), encoding="utf-8")
    print(f"{args.out}: {bpm:g} BPM, {len(markers)} markers, {len(project.findall('TRACK'))} tracks, {length_seconds:.2f} s")


if __name__ == "__main__":
    main()
