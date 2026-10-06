import argparse
import json

import librosa
import numpy as np

BINS = 8
SR = 22050
HOP = 256
N_FFT = 2048
FAST_GAP = 0.35
SLOW_GAP = 0.15


def band_env(spec, freqs, lo, hi):
    band = (freqs >= lo) & (freqs < hi)
    flux = np.maximum(0, np.diff(np.log1p(spec[band]), axis=1)).sum(axis=0)
    return np.concatenate([[0], flux]) / (flux.max() + 1e-9)


def fold(env, frame_t, beats):
    phase = np.zeros(BINS)
    phase_n = np.zeros(BINS)
    bar = np.zeros(4)
    bar_n = np.zeros(4)
    for i in range(len(beats) - 1):
        start, end = beats[i], beats[i + 1]
        inside = (frame_t >= start) & (frame_t < end)
        if not inside.any():
            continue
        idx = np.minimum(((frame_t[inside] - start) / (end - start) * BINS).astype(int), BINS - 1)
        np.add.at(phase, idx, env[inside])
        np.add.at(phase_n, idx, 1)
        head = inside & (frame_t < start + (end - start) / BINS)
        bar[i % 4] += env[head].mean() if head.any() else 0
        bar_n[i % 4] += 1
    return phase / np.maximum(phase_n, 1), bar / np.maximum(bar_n, 1)


def circular_phase(profile):
    angles = 2 * np.pi * np.arange(BINS) / BINS
    weights = (profile - profile.mean()).clip(0)
    return float(np.angle(np.sum(weights * np.exp(1j * angles))) / (2 * np.pi)) % 1


def verdict_of(gap):
    if gap > FAST_GAP:
        return "fast"
    if gap < SLOW_GAP:
        return "slow"
    return "undecided"


def rounded(values):
    return [round(float(v), 3) for v in values]


def analyse(y, sr, slow):
    _, percussive = librosa.effects.hpss(y)
    spec = np.abs(librosa.stft(percussive, n_fft=N_FFT, hop_length=HOP))
    freqs = librosa.fft_frequencies(sr=sr, n_fft=N_FFT)
    frame_t = librosa.frames_to_time(np.arange(spec.shape[1]), sr=sr, hop_length=HOP)
    kick = band_env(spec, freqs, 30, 150)
    snare = band_env(spec, freqs, 180, 5000)
    onset = librosa.onset.onset_strength(y=percussive, sr=sr, hop_length=HOP)
    _, beats = librosa.beat.beat_track(
        onset_envelope=onset, sr=sr, hop_length=HOP, start_bpm=slow, tightness=400, units="time"
    )
    kick_phase, kick_bar = fold(kick, frame_t, beats)
    snare_phase, snare_bar = fold(snare, frame_t, beats)
    gap = abs((circular_phase(snare_phase) - circular_phase(kick_phase) + 0.5) % 1 - 0.5)
    tempogram = librosa.feature.tempogram(onset_envelope=onset, sr=sr, hop_length=HOP).mean(axis=1)
    tempo_axis = librosa.tempo_frequencies(len(tempogram), sr=sr, hop_length=HOP)
    strength = lambda bpm: float(tempogram[np.argmin(np.abs(tempo_axis - bpm))])
    verdict = verdict_of(gap)
    return {
        "verdict": verdict,
        "bpm": {"fast": slow * 2, "slow": slow, "undecided": None}[verdict],
        "slow_bpm": slow,
        "fast_bpm": slow * 2,
        "grid_bpm": round(60 / float(np.median(np.diff(beats))), 2) if len(beats) > 2 else None,
        "kick_snare_phase_gap": round(gap, 3),
        "kick_phase": rounded(kick_phase),
        "snare_phase": rounded(snare_phase),
        "kick_by_slow_beat": rounded(kick_bar),
        "snare_by_slow_beat": rounded(snare_bar),
        "tempogram_slow_fast": rounded([strength(slow), strength(slow * 2)]),
    }


def parse_args(argv=None):
    parser = argparse.ArgumentParser(
        prog="uv run tempo_octave.py",
        description=(
            "Decide whether an audio file runs at SLOW_BPM or twice that, from the backbeat. "
            "Kick and snare onsets are folded onto the slow beat grid: a true fast tempo puts the "
            f"snare half a slow beat from the kick (gap > {FAST_GAP}), a true slow tempo puts both "
            f"on slow beat heads (gap < {SLOW_GAP}). Prints JSON with the verdict and its evidence."
        ),
    )
    parser.add_argument("audio", help="path to an audio file readable by librosa")
    parser.add_argument("slow_bpm", type=float, help="the lower of the two candidate tempos")
    parser.add_argument(
        "--section",
        nargs=2,
        type=float,
        metavar=("START", "END"),
        help="also analyse only START..END seconds, such as a chorus with a steady beat",
    )
    return parser.parse_args(argv)


def main(argv=None):
    args = parse_args(argv)
    y, sr = librosa.load(args.audio, sr=SR, mono=True)
    result = {"whole": analyse(y, sr, args.slow_bpm)}
    if args.section:
        start, end = args.section
        result["section"] = analyse(y[int(start * sr) : int(end * sr)], sr, args.slow_bpm)
    print(json.dumps(result, ensure_ascii=False))


if __name__ == "__main__":
    main()
