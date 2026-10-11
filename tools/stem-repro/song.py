import json
from pathlib import Path

import soundfile as sf

from audio import SR, load
from midi_notes import notes


class Song:
    def __init__(self, config):
        self.path = Path(config).resolve()
        self.root = self.path.parent
        self.cfg = json.loads(self.path.read_text())
        self.bpm = self.cfg["bpm"]
        self.bar = 60 / self.bpm * self.cfg.get("beats_per_bar", 4)
        self.beat = 60 / self.bpm
        self.stem_names = self.cfg["stem_names"]
        self.sections = self.cfg.get("sections", {})
        self.parts = self.cfg.get("parts", {})

    def file(self, rel):
        return self.root / rel

    def stem_path(self, name):
        return self.file(self.cfg["stems"].format(name=name))

    def frames(self):
        return sf.info(self.file(self.cfg["premaster"])).frames

    def stem(self, name, n=None):
        return load(self.stem_path(name), n)

    def section(self, x, name):
        return self.bars(x, *self.sections[name])

    def bars(self, x, b0, b1):
        return x[int(b0 * self.bar * SR):int(b1 * self.bar * SR)]

    def part_notes(self, part):
        spec = self.parts[part]
        return notes(self.file(spec["midi"]), spec.get("track"))

    def notes_in(self, lst, b0, b1):
        return [n for n in lst if b0 * self.bar - 0.01 <= n[0] < b1 * self.bar - 0.01]

