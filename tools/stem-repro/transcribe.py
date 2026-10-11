import sys
from pathlib import Path

from basic_pitch.inference import predict


def main(out_dir, bpm, *stems):
    Path(out_dir).mkdir(parents=True, exist_ok=True)
    for stem in stems:
        _, midi, found = predict(stem, onset_threshold=0.5, frame_threshold=0.3, minimum_note_length=58, midi_tempo=float(bpm))
        midi.write(str(Path(out_dir) / (Path(stem).stem + ".mid")))
        print(Path(stem).name, len(found), flush=True)


if __name__ == "__main__":
    main(*sys.argv[1:])
