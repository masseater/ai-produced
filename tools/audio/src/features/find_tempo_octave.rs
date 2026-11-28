use std::path::PathBuf;

use crate::shared::audio_file::decode;
use crate::shared::statistics::{mean, round};
use anyhow::{Context, Result};
use clap::Parser;
use ndarray::{Array1, Array2, ArrayView1, Axis, Slice};
use num_complex::Complex64;
use num_traits::ToPrimitive;
use serde_json::{Value, json};
use sonara::beat::beat_track;
use sonara::core::convert::{fft_frequencies, tempo_frequencies};
use sonara::core::spectrum::stft_power;
use sonara::effects::hpss;
use sonara::feature::rhythm::tempogram;
use sonara::onset::onset_strength;
use sonara::{PadMode, WindowSpec};

const BINS: u8 = 8;
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

fn fold(env: &[f64], frame_t: &[f64], beats: &[f64]) -> Result<(Vec<f64>, Vec<f64>)> {
    let bins = f64::from(BINS);
    let last_bin = usize::from(BINS) - 1;
    let mut phase = vec![0.0f64; usize::from(BINS)];
    let mut phase_n = vec![0.0f64; usize::from(BINS)];
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
            let idx = ((frame_t[f] - start) / (end - start) * bins)
                .to_usize()
                .context("phase bin")?
                .min(last_bin);
            phase[idx] += env[f];
            phase_n[idx] += 1.0;
        }
        let head: Vec<f64> = inside
            .iter()
            .filter(|&&f| frame_t[f] < start + (end - start) / bins)
            .map(|&f| env[f])
            .collect();
        if !head.is_empty() {
            bar[i % 4] += mean(head)?;
        }
        bar_n[i % 4] += 1.0;
    }
    Ok((
        phase
            .iter()
            .zip(&phase_n)
            .map(|(p, n)| p / n.max(1.0))
            .collect(),
        bar.iter()
            .zip(&bar_n)
            .map(|(b, n)| b / n.max(1.0))
            .collect(),
    ))
}

fn circular_phase(profile: &[f64]) -> Result<f64> {
    let average = mean(profile.iter().copied())?;
    let sum: Complex64 = (0u8..)
        .zip(profile)
        .map(|(k, p)| {
            let angle = 2.0 * std::f64::consts::PI * f64::from(k) / f64::from(BINS);
            Complex64::from_polar((p - average).max(0.0), angle)
        })
        .sum();
    Ok((sum.arg() / (2.0 * std::f64::consts::PI)).rem_euclid(1.0))
}

fn seconds(frame: usize) -> Result<f64> {
    Ok((frame * HOP).to_f64().context("frame time")? / f64::from(SR))
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
    let sr = SR.to_f32().context("sample rate")?;
    let freqs = fft_frequencies(sr, N_FFT);
    let frame_t: Vec<f64> = (0..spec.ncols()).map(seconds).collect::<Result<_>>()?;
    let kick = band_env(&spec, &freqs, 30.0, 150.0);
    let snare = band_env(&spec, &freqs, 180.0, 5000.0);
    let onset = onset_strength(percussive.view(), SR, HOP)?;
    let (_, beat_frames) = beat_track(
        None,
        Some(onset.view()),
        SR,
        HOP,
        slow.to_f32().context("slow bpm")?,
        400.0,
        true,
    )?;
    let beats: Vec<f64> = beat_frames
        .iter()
        .map(|f| seconds(*f))
        .collect::<Result<_>>()?;
    let (kick_phase, kick_bar) = fold(&kick, &frame_t, &beats)?;
    let (snare_phase, snare_bar) = fold(&snare, &frame_t, &beats)?;
    let gap = ((circular_phase(&snare_phase)? - circular_phase(&kick_phase)? + 0.5)
        .rem_euclid(1.0)
        - 0.5)
        .abs();
    let win = 384.min(onset.len());
    let gram = tempogram(None, Some(onset.view()), SR, HOP, win)?
        .mean_axis(Axis(1))
        .unwrap_or_else(|| Array1::zeros(win));
    let axis = tempo_frequencies(gram.len(), HOP, sr);
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

pub fn run() -> Result<()> {
    println!("{}", report(&Args::parse())?);
    Ok(())
}

fn report(args: &Args) -> Result<String> {
    let y = decode(&args.audio)?.mono(SR)?;
    let mut result = json!({ "whole": analyse(&y.view(), args.slow_bpm)? });
    if let Some(section) = &args.section {
        let rate = f64::from(SR);
        let start = (section[0] * rate)
            .to_usize()
            .context("bad section start")?
            .min(y.len());
        let end = (section[1] * rate)
            .to_usize()
            .context("bad section end")?
            .clamp(start, y.len());
        result["section"] = analyse(
            &y.slice_axis(Axis(0), Slice::from(start..end)),
            args.slow_bpm,
        )?;
    }
    Ok(serde_json::to_string(&result)?)
}

#[cfg(test)]
mod tests {
    use std::f64::consts::PI;

    use serde_json::Value;

    use super::*;
    use crate::shared::test_signal::{near, write_wav};

    fn drums(rate: u32, seconds: u32, slow_bpm: f64, snare_offset: f64) -> Vec<f64> {
        let beat = 60.0 / slow_bpm;
        let mut seed = 12345u64;
        let mut out = vec![0.0; (rate * seconds) as usize];
        for (i, sample) in (0u32..).zip(out.iter_mut()) {
            let t = f64::from(i) / f64::from(rate);
            let since_kick = t % beat;
            let since_snare = (t - snare_offset * beat).rem_euclid(beat);
            seed = seed
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            let noise = f64::from(u32::try_from(seed >> 33).unwrap()) / f64::from(1u32 << 31) - 1.0;
            let kick = (2.0 * PI * 60.0 * since_kick).sin() * (-since_kick / 0.12).exp();
            let snare = noise * (-since_snare / 0.08).exp();
            *sample = 0.5 * kick + 0.3 * snare;
        }
        out
    }

    #[test]
    fn reads_the_backbeat() {
        let dir = tempfile::tempdir().unwrap();
        let rate = 22050;
        for (offset, verdict) in [(0.5, "fast"), (0.0, "slow")] {
            let path = dir.path().join(format!("drums-{verdict}.wav"));
            write_wav(&path, rate, &[drums(rate, 30, 80.0, offset)]);
            let args = Args::parse_from([
                "tempo-octave",
                path.to_str().unwrap(),
                "80",
                "--section",
                "5",
                "25",
            ]);
            let result: Value = serde_json::from_str(&report(&args).unwrap()).unwrap();
            assert_eq!(result["whole"]["verdict"], verdict, "{result}");
            assert_eq!(result["section"]["verdict"], verdict, "{result}");
            near(&result["whole"]["grid_bpm"], 80.0, 1.0);
        }
    }
}
