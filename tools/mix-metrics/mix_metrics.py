import argparse
import json
import re
import subprocess
import sys
import tempfile
from pathlib import Path

import librosa
import numpy as np

REFS = Path(__file__).resolve().parents[2] / "docs/wiki/data/taste-refs-v1.json"
BANDS = {"sub": (20, 60), "low": (60, 250), "mid": (250, 2000), "high_mid": (2000, 6000), "high": (6000, 22050)}
KEYS = [
    "bpm", "lufs_integrated", "lra", "crest_db", "stereo_side_to_mid_db", "onsets_per_s",
    "spectral_centroid_hz", "loudness_p90_minus_p10_db", "sub", "low", "mid", "high_mid", "high",
]
MARGIN = 0.25


def ebur128(path):
    err = subprocess.run(
        ["ffmpeg", "-hide_banner", "-nostats", "-i", str(path), "-af", "ebur128=peak=true:framelog=quiet", "-f", "null", "-"],
        capture_output=True, text=True,
    ).stderr
    summary = err[err.rfind("Summary:"):]
    def value(key):
        return float(re.search(rf"{key}:\s+(-?[\d.]+|-inf)", summary).group(1))

    return value("I"), value("LRA"), value("Peak")


def encoded(path, workdir):
    out = Path(workdir) / "encoded.wav"
    opus = Path(workdir) / "encoded.opus"
    subprocess.run(["ffmpeg", "-loglevel", "error", "-y", "-i", str(path), "-c:a", "libopus", "-b:a", "160k", str(opus)], check=True)
    subprocess.run(["ffmpeg", "-loglevel", "error", "-y", "-i", str(opus), "-ar", "48000", str(out)], check=True)
    return out


def measure(path, bpm=None, offset=0.0, duration=None):
    y2, sr = librosa.load(path, sr=44100, mono=False, offset=offset, duration=duration)
    if y2.ndim == 1:
        y2 = np.vstack([y2, y2])
    y = y2.mean(0)
    lufs, lra, true_peak = ebur128(path) if offset == 0 and duration is None else (None, None, None)
    power = np.abs(librosa.stft(y, n_fft=4096, hop_length=2048)) ** 2
    freqs = librosa.fft_frequencies(sr=sr, n_fft=4096)
    total = power[freqs >= 20].sum()
    mid, side = (y2[0] + y2[1]) / 2, (y2[0] - y2[1]) / 2
    rms_db = 20 * np.log10(librosa.feature.rms(y=y, frame_length=2048, hop_length=1024)[0] + 1e-9)
    voiced = rms_db[rms_db > -80]
    y22 = librosa.resample(y, orig_sr=sr, target_sr=22050)
    onsets = librosa.onset.onset_detect(y=y22, sr=22050, units="time")
    seconds = len(y) / sr
    result = {
        "bpm": bpm,
        "lufs_integrated": lufs,
        "lra": lra,
        "true_peak_dbtp": true_peak,
        "crest_db": round(float(20 * np.log10(np.abs(y2).max() / np.sqrt((y**2).mean()))), 2),
        "stereo_side_to_mid_db": round(float(10 * np.log10((side**2).mean() / (mid**2).mean() + 1e-12)), 2),
        "onsets_per_s": round(len(onsets) / seconds, 2),
        "spectral_centroid_hz": round(float(librosa.feature.spectral_centroid(y=y22, sr=22050)[0].mean()), 1),
        "loudness_p90_minus_p10_db": round(float(np.percentile(voiced, 90) - np.percentile(voiced, 10)), 2) if len(voiced) else None,
        "rms_dbfs": round(float(10 * np.log10((y**2).mean() + 1e-12)), 2),
    }
    for name, (lo, hi) in BANDS.items():
        result[name] = round(float(10 * np.log10(power[(freqs >= lo) & (freqs < hi)].sum() / total)), 2)
    return result


def band():
    tracks = json.loads(REFS.read_text())["tracks"]
    out = {}
    for key in KEYS:
        values = [t[key] for t in tracks]
        lo, hi = min(values), max(values)
        margin = (hi - lo) * MARGIN
        out[key] = (lo - margin, hi + margin)
    return out


def judge(metrics):
    rows = []
    for key, (lo, hi) in band().items():
        value = metrics.get(key)
        ok = value is not None and lo <= value <= hi
        rows.append({"key": key, "value": value, "lo": round(lo, 2), "hi": round(hi, 2), "in_band": ok})
    return rows


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("wav", type=Path)
    parser.add_argument("--bpm", type=float)
    parser.add_argument("--offset", type=float, default=0.0)
    parser.add_argument("--duration", type=float)
    parser.add_argument("--judge", action="store_true")
    args = parser.parse_args()
    raw = measure(args.wav, args.bpm, args.offset, args.duration)
    if not args.judge:
        json.dump(raw, sys.stdout, ensure_ascii=False)
        print()
        return
    with tempfile.TemporaryDirectory() as workdir:
        coded = measure(encoded(args.wav, workdir), args.bpm)
    rows = judge(coded)
    gates = {
        "true_peak_dbtp<=-1.0": raw["true_peak_dbtp"] <= -1.0,
        "no_clip": raw["true_peak_dbtp"] < 0,
        "stereo_not_cancelled": raw["stereo_side_to_mid_db"] < 0,
    }
    report = {
        "raw": raw,
        "encoded_opus160": coded,
        "band": rows,
        "out_of_band": [r["key"] for r in rows if not r["in_band"]],
        "verdict": "in_taste" if sum(not r["in_band"] for r in rows) <= 2 else "warning",
        "gates": gates,
    }
    json.dump(report, sys.stdout, ensure_ascii=False, indent=1)
    print()


if __name__ == "__main__":
    main()
