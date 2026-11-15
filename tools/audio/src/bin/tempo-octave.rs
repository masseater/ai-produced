use std::path::PathBuf;

use anyhow::Result;
use audio::{decode, round};
use clap::Parser;
use ndarray::{Array1, Array2, ArrayView1, Axis, Slice};
use num_complex::Complex64;
use serde_json::{Value, json};
use sonara::beat::beat_track;
use sonara::core::convert::{fft_frequencies, tempo_frequencies};
use sonara::core::spectrum::stft_power;
use sonara::effects::hpss;
use sonara::feature::rhythm::tempogram;
use sonara::onset::onset_strength;
use sonara::{PadMode, WindowSpec};

const BINS: usize = 8;
const SR: u32 = 22050;
const HOP: usize = 256;
const N_FFT: usize = 2048;
const FAST_GAP: f64 = 0.35;
const SLOW_GAP: f64 = 0.15;

#[derive(Parser)]
#[command(
    about = "Decide whether an audio file runs at SLOW_BPM or twice that, from the backbeat. Kick and snare onsets are folded onto the slow beat grid: a true fast tempo puts the snare half a slow beat from the kick (gap > 0.35), a true slow tempo puts both on slow beat heads (gap < 0.15). Prints JSON with the verdict and its evidence."
)]
struct Args {
    #[arg(help = "path to an audio file")]
    audio: PathBuf,
    #[arg(help = "the lower of the two candidate tempos")]
    slow_bpm: f64,
    #[arg(
        long,
        num_args = 2,
        value_names = ["START", "END"],
        help = "also analyse only START..END seconds, such as a chorus with a steady beat"
    )]
    section: Option<Vec<f64>>,
}

fn band_env(spec: &Array2<f32>, freqs: &Array1<f32>, lo: f32, hi: f32) -> Vec<f64> {
    let frames = spec.ncols();
    let mut flux = vec![0.0f64; frames];
    for (bin, freq) in freqs.iter().enumerate() {
        if *freq < lo || *freq >= hi {
            continue;
        }
        let row = spec.row(bin);
        for t in 1..frames {
            let rise = f64::from(row[t]).ln_1p() - f64::from(row[t - 1]).ln_1p();
            flux[t] += rise.max(0.0);
        }
    }
    let max = flux.iter().copied().fold(0.0, f64::max);
    flux.iter().map(|v| v / (max + 1e-9)).collect()
}

fn fold(env: &[f64], frame_t: &[f64], beats: &[f64]) -> (Vec<f64>, Vec<f64>) {
    let mut phase = [0.0f64; BINS];
    let mut phase_n = [0.0f64; BINS];
    let mut bar = [0.0f64; 4];
    let mut bar_n = [0.0f64; 4];
    for (i, pair) in beats.windows(2).enumerate() {
        let (start, end) = (pair[0], pair[1]);
        let inside: Vec<usize> = (0..frame_t.len())
            .filter(|&f| frame_t[f] >= start && frame_t[f] < end)
            .collect();
        if inside.is_empty() {
            continue;
        }
        for &f in &inside {
            let idx = (((frame_t[f] - start) / (end - start) * BINS as f64) as usize).min(BINS - 1);
            phase[idx] += env[f];
            phase_n[idx] += 1.0;
        }
        let head: Vec<f64> = inside
            .iter()
            .filter(|&&f| frame_t[f] < start + (end - start) / BINS as f64)
            .map(|&f| env[f])
            .collect();
        if !head.is_empty() {
            bar[i % 4] += head.iter().sum::<f64>() / head.len() as f64;
        }
        bar_n[i % 4] += 1.0;
    }
    (
        phase
            .iter()
            .zip(&phase_n)
            .map(|(p, n)| p / n.max(1.0))
            .collect(),
        bar.iter()
            .zip(&bar_n)
            .map(|(b, n)| b / n.max(1.0))
            .collect(),
    )
}

fn circular_phase(profile: &[f64]) -> f64 {
    let average = profile.iter().sum::<f64>() / profile.len() as f64;
    let sum: Complex64 = profile
        .iter()
        .enumerate()
        .map(|(k, p)| {
            let angle = 2.0 * std::f64::consts::PI * k as f64 / BINS as f64;
            Complex64::from_polar((p - average).max(0.0), angle)
        })
        .sum();
    (sum.arg() / (2.0 * std::f64::consts::PI)).rem_euclid(1.0)
}

fn verdict_of(gap: f64) -> &'static str {
    if gap > FAST_GAP {
        "fast"
    } else if gap < SLOW_GAP {
        "slow"
    } else {
        "undecided"
    }
}

fn rounded(values: &[f64]) -> Vec<f64> {
    values.iter().map(|v| round(*v, 3)).collect()
}

fn median(values: &mut [f64]) -> f64 {
    values.sort_by(f64::total_cmp);
    let n = values.len();
    if n % 2 == 1 {
        values[n / 2]
    } else {
        f64::midpoint(values[n / 2 - 1], values[n / 2])
    }
}

fn analyse(y: &ArrayView1<f32>, slow: f64) -> Result<Value> {
    let (_, percussive) = hpss(y.view(), 31, 1.0)?;
    let window = WindowSpec::Named("hann".into());
    let spec = stft_power(
        percussive.view(),
        N_FFT,
        HOP,
        &window,
        true,
        PadMode::Constant,
        1.0,
    )?;
    let freqs = fft_frequencies(SR as f32, N_FFT);
    let frame_t: Vec<f64> = (0..spec.ncols())
        .map(|f| (f * HOP) as f64 / f64::from(SR))
        .collect();
    let kick = band_env(&spec, &freqs, 30.0, 150.0);
    let snare = band_env(&spec, &freqs, 180.0, 5000.0);
    let onset = onset_strength(percussive.view(), SR, HOP)?;
    let (_, beat_frames) = beat_track(None, Some(onset.view()), SR, HOP, slow as f32, 400.0, true)?;
    let beats: Vec<f64> = beat_frames
        .iter()
        .map(|f| (f * HOP) as f64 / f64::from(SR))
        .collect();
    let (kick_phase, kick_bar) = fold(&kick, &frame_t, &beats);
    let (snare_phase, snare_bar) = fold(&snare, &frame_t, &beats);
    let gap = ((circular_phase(&snare_phase) - circular_phase(&kick_phase) + 0.5).rem_euclid(1.0)
        - 0.5)
        .abs();
    let win = 384.min(onset.len());
    let gram = tempogram(None, Some(onset.view()), SR, HOP, win)?
        .mean_axis(Axis(1))
        .unwrap_or_else(|| Array1::zeros(win));
    let axis = tempo_frequencies(gram.len(), HOP, SR as f32);
    let strength = |bpm: f64| -> f64 {
        let nearest = (1..axis.len())
            .min_by(|a, b| {
                (f64::from(axis[*a]) - bpm)
                    .abs()
                    .total_cmp(&(f64::from(axis[*b]) - bpm).abs())
            })
            .unwrap_or(0);
        f64::from(gram[nearest])
    };
    let verdict = verdict_of(gap);
    let mut spacing: Vec<f64> = beats.windows(2).map(|p| p[1] - p[0]).collect();
    Ok(json!({
        "verdict": verdict,
        "bpm": match verdict { "fast" => Some(slow * 2.0), "slow" => Some(slow), _ => None },
        "slow_bpm": slow,
        "fast_bpm": slow * 2.0,
        "grid_bpm": (beats.len() > 2).then(|| round(60.0 / median(&mut spacing), 2)),
        "kick_snare_phase_gap": round(gap, 3),
        "kick_phase": rounded(&kick_phase),
        "snare_phase": rounded(&snare_phase),
        "kick_by_slow_beat": rounded(&kick_bar),
        "snare_by_slow_beat": rounded(&snare_bar),
        "tempogram_slow_fast": rounded(&[strength(slow), strength(slow * 2.0)]),
    }))
}

fn main() -> Result<()> {
    let args = Args::parse();
    let y = decode(&args.audio)?.mono(SR)?;
    let mut result = json!({ "whole": analyse(&y.view(), args.slow_bpm)? });
    if let Some(section) = &args.section {
        let rate = f64::from(SR);
        let start = ((section[0] * rate) as usize).min(y.len());
        let end = ((section[1] * rate) as usize).clamp(start, y.len());
        result["section"] = analyse(
            &y.slice_axis(Axis(0), Slice::from(start..end)),
            args.slow_bpm,
        )?;
    }
    println!("{}", serde_json::to_string(&result)?);
    Ok(())
}
