import os
import re
import struct

import numpy as np
import pedalboard

from audio import SR

ALPHABET = ".ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+"
VST3 = os.environ.get("SURGE_VST3", "/opt/surge/Surge XT.vst3")
PATCHES = os.environ.get("SURGE_PATCHES", "/usr/share/surge-xt")
COMPONENT = re.compile(r"<IComponent>([^<]*)</IComponent>")


def juce_decode(s):
    size, enc = s.split(".", 1)
    size = int(size)
    out = bytearray(size)
    bit = 0
    for ch in enc:
        v = ALPHABET.index(ch)
        for b in range(6):
            if bit >= size * 8:
                break
            if v >> b & 1:
                out[bit >> 3] |= 1 << (bit & 7)
            bit += 1
    return bytes(out)


def juce_encode(data):
    bits = len(data) * 8
    out = []
    for i in range(0, bits, 6):
        v = 0
        for b in range(6):
            k = i + b
            if k < bits and data[k >> 3] >> (k & 7) & 1:
                v |= 1 << b
        out.append(ALPHABET[v])
    return f"{len(data)}." + "".join(out)


def with_patch(raw_state, fxp):
    xml = raw_state[8:].decode("utf-8", errors="ignore")
    body = xml.replace(COMPONENT.search(xml).group(1), juce_encode(fxp[60:])).encode("utf-8")
    return b"VC2!" + struct.pack("<I", len(body)) + body


class Surge:
    def __init__(self):
        self.plugin = pedalboard.load_plugin(VST3)
        self.base = self.plugin.raw_state

    def load(self, fxp_path, params=None):
        with open(fxp_path, "rb") as f:
            self.plugin.raw_state = with_patch(self.base, f.read())
        self.plugin([], duration=0.3, sample_rate=SR, reset=False)
        for k, v in (params or {}).items():
            self.plugin.parameters[k].raw_value = float(v)

    def render(self, events, duration):
        a = self.plugin(events, duration=duration, sample_rate=SR, reset=False)
        self.plugin([], duration=0.0, sample_rate=SR, reset=True)
        return a.T

    def render_mono(self, events, duration, n):
        m = self.render(events, duration).mean(1)[:n]
        if not np.isfinite(m).all() or np.abs(m).max() < 1e-5:
            return None
        return m
