import argparse
import json
import sys
import tomllib
from pathlib import Path

import librosa
import numpy as np

SR = 22050
HOP = 256


def spans(score):
    seconds_per_bar = 4 * 60 / score["bpm"]
    bar = 0
    for section in score["sections"]:
        yield section["name"], bar * seconds_per_bar, (bar + section["bars"]) * seconds_per_bar
        bar += section["bars"]


def describe(y):
    rms = librosa.feature.rms(y=y, hop_length=HOP)[0]
    loud = rms > 10 ** (-40 / 20)
    if not loud.any():
        return None
    f0, voiced, _ = librosa.pyin(y, fmin=150, fmax=900, sr=SR, hop_length=HOP)
    voiced = voiced[: len(loud)] & loud[: len(voiced)]
    cents = 1200 * np.log2(f0[: len(voiced)][voiced] / 440)
    detrended = cents - np.convolve(cents, np.ones(15) / 15, mode="same") if len(cents) > 15 else cents
    return {
        "rms_db": round(float(20 * np.log10(np.sqrt((rms[loud] ** 2).mean()))), 1),
        "centroid_hz": round(float(librosa.feature.spectral_centroid(y=y, sr=SR, hop_length=HOP)[0][loud].mean())),
        "flatness": round(float(librosa.feature.spectral_flatness(y=y, hop_length=HOP)[0][loud].mean()), 4),
        "sound_ratio": round(float(loud.mean()), 2),
        "voiced_of_sound": round(float(voiced.sum() / loud.sum()), 2),
        "pitch_wobble_cents": round(float(np.abs(detrended[7:-7]).mean()), 1) if len(detrended) > 15 else None,
    }


def main():
    parser = argparse.ArgumentParser(description="Per-section vocal description: level, brightness, noisiness, gaps, pitch wobble")
    parser.add_argument("wav", type=Path)
    parser.add_argument("--score", type=Path, required=True)
    args = parser.parse_args()
    score = tomllib.loads(args.score.read_text(encoding="utf-8"))
    y, _ = librosa.load(args.wav, sr=SR, mono=True)
    out = {}
    for name, start, end in spans(score):
        out[name] = describe(y[int(start * SR): int(end * SR)])
    json.dump(out, sys.stdout, ensure_ascii=False, indent=1)
    print()


if __name__ == "__main__":
    main()
