use std::f64::consts::PI;
use std::path::Path;

use anyhow::{Context, Result, bail};
use ndarray::ArrayView1;
use num_traits::ToPrimitive;
use rand::SeedableRng;
use rand::rngs::StdRng;
use rand_distr::{Distribution, StandardNormal};
use serde_json::{Map, Value, json};
use sonara::onset::onset_detect;

use crate::shared::eq::matched;
use crate::shared::optim::differential_evolution;
use crate::shared::placement::rounded;
use crate::shared::signal::{
    Format, Frame, RATE, SR, correlate_full, correlate_valid, dual, interp, mean, mean_square,
    mono, percentile, rms, round, smooth3_edge, stft, write,
};
use crate::shared::song::Song;
use crate::shared::spectral::dist;

const NAMES: [&str; 15] = [
    "f_end",
    "f_a",
    "tau_a",
    "f_b",
    "tau_b",
    "attack",
    "hold",
    "release",
    "drive",
    "knock_f",
    "knock_tau",
    "knock_lvl",
    "click",
    "click_tau",
    "phase",
];
const BOUNDS: [(f64, f64); 15] = [
    (30.0, 60.0),
    (0.0, 800.0),
    (0.001, 0.06),
    (0.0, 200.0),
    (0.01, 0.3),
    (0.0002, 0.02),
    (0.0, 0.35),
    (0.005, 0.15),
    (0.3, 6.0),
    (300.0, 3000.0),
    (0.001, 0.05),
    (0.0, 1.5),
    (0.0, 2.0),
    (0.0005, 0.01),
    (0.0, 2.0 * PI),
];

#[must_use]
pub fn kick(p: &[f64], n: usize) -> Vec<f64> {
    let [
        f_end,
        f_a,
        tau_a,
        f_b,
        tau_b,
        attack,
        hold,
        release,
        drive,
        kf,
        kt,
        kl,
        cl,
        ct,
        phase,
    ] = p[..15].try_into().expect("15 kick parameters");
    let mut rng = StdRng::seed_from_u64(0);
    let mut cycles = 0.0;
    let mut last_click = 0.0;
    (0u32..)
        .take(n)
        .map(|i| {
            let time = f64::from(i) / RATE;
            cycles += f_end + f_a * (-time / tau_a).exp() + f_b * (-time / tau_b).exp();
            let amp = (time / attack).clamp(0.0, 1.0)
                * if time < hold {
                    1.0
                } else {
                    (-(time - hold) / release).exp()
                };
            let body = (phase + 2.0 * PI * cycles / RATE).sin() * amp;
            let knock = kl * (2.0 * PI * kf * time).sin() * (-time / kt).exp();
            let noise: f64 = StandardNormal.sample(&mut rng);
            let click = noise * (-time / ct).exp() * cl;
            let sample = body + knock + click - last_click;
            last_click = click;
            (drive * sample).tanh() / drive.tanh()
        })
        .collect()
}

fn samples(seconds: f64) -> Result<usize> {
    (seconds * RATE)
        .to_usize()
        .with_context(|| format!("bad duration {seconds} s"))
}

fn template(m: &[f64], length: f64, hits: usize) -> Result<Vec<f64>> {
    let n = samples(length)?;
    let y: Vec<f32> = m
        .iter()
        .map(|v| v.to_f32().context("sample"))
        .collect::<Result<_>>()?;
    let starts: Vec<usize> =
        onset_detect(Some(ArrayView1::from(&y)), None, SR, 512, true, 0.08, 0)?
            .iter()
            .map(|f| f * 512)
            .collect();
    let shots: Vec<&[f64]> = starts
        .iter()
        .filter(|o| *o + n < m.len())
        .take(hits)
        .map(|o| &m[*o..*o + n])
        .collect();
    let Some(first) = shots.first() else {
        bail!("no kick onsets found");
    };
    let aligned: Vec<Vec<f64>> = shots
        .iter()
        .map(|s| {
            let c = correlate_full(s, first)?;
            let window = &c[n - 1 - 441..n + 441];
            let shift = window
                .iter()
                .enumerate()
                .max_by(|a, b| a.1.total_cmp(b.1))
                .map_or(0, |(i, _)| i);
            Ok((0..n).map(|i| s[(i + n + shift - 441) % n]).collect())
        })
        .collect::<Result<_>>()?;
    let t: Vec<f64> = (0..n)
        .map(|i| {
            let column: Vec<f64> = aligned.iter().map(|a| a[i]).collect();
            percentile(&column, 50.0)
        })
        .collect::<Result<_>>()?;
    let top = t.iter().fold(0.0f64, |a, v| a.max(v.abs()));
    let onset = t.iter().position(|v| v.abs() > 0.1 * top).unwrap_or(0);
    let start = onset.saturating_sub(samples(0.001)?);
    let mut out = t[start..].to_vec();
    out.extend(std::iter::repeat_n(0.0, start));
    Ok(out)
}

fn spec(y: &[f64], n: usize) -> Result<Vec<f64>> {
    Ok(stft(y, n)?
        .frames
        .iter()
        .flatten()
        .map(|c| 20.0 * (c.norm() + 1e-6).log10())
        .collect())
}

fn env2ms(y: &[f64]) -> Vec<f64> {
    let e: Vec<f64> = y
        .as_chunks::<88>()
        .0
        .iter()
        .map(|c| 10.0 * (mean_square(c) + 1e-12).log10())
        .collect();
    let top = e.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    e.iter().map(|v| (v - top).max(-60.0)).collect()
}

fn fit(reference: &[f64], max_iter: u64, seed: u64) -> Result<(f64, Vec<f64>)> {
    let n = reference.len();
    let sizes = [256, 1024, 4096];
    let targets: Vec<(usize, Vec<f64>, f64)> = sizes
        .iter()
        .map(|k| {
            let r = spec(reference, *k)?;
            let floor = r.iter().copied().fold(f64::NEG_INFINITY, f64::max) - 80.0;
            Ok((*k, r.iter().map(|v| v.max(floor)).collect(), floor))
        })
        .collect::<Result<_>>()?;
    let target_env = env2ms(reference);
    let level = rms(reference);
    let loss = |p: &[f64]| -> Result<f64> {
        let y = kick(p, n);
        let gain = level / (rms(&y) + 1e-12);
        let y: Vec<f64> = y.iter().map(|v| v * gain).collect();
        let spectral = targets
            .iter()
            .map(|(k, r, floor)| {
                let c = spec(&y, *k)?;
                mean(c.iter().zip(r).map(|(a, b)| (a.max(*floor) - b).abs()))
            })
            .collect::<Result<Vec<f64>>>()?;
        let e = env2ms(&y);
        Ok(mean(spectral)? + mean(e.iter().zip(&target_env).map(|(a, b)| (a - b).abs()))?)
    };
    let best =
        differential_evolution(|p| loss(p).unwrap_or(f64::NAN), &BOUNDS, 15, max_iter, seed)?;
    Ok((best.f, best.x))
}

fn hits(m: &[f64], tmpl: &[f64], grid: f64) -> Result<Vec<(usize, f64)>> {
    let width = samples(0.12)?;
    let gap = samples(0.05)?;
    let count = (m.len().to_f64().context("length")? / (grid * RATE))
        .to_usize()
        .context("grid")?;
    let probe = &tmpl[..width.min(tmpl.len())];
    let energy: f64 = probe.iter().map(|v| v * v).sum();
    let probe_norm = energy.sqrt();
    let mut out: Vec<(usize, f64)> = Vec::new();
    for step in (0u32..).take(count) {
        let at = (f64::from(step) * grid * RATE)
            .round_ties_even()
            .to_usize()
            .context("grid")?;
        let lo = at.saturating_sub(441);
        let seg = &m[lo.min(m.len())..(at + 441 + width).min(m.len())];
        if seg.len() < width + 882 || rms(seg) < 1e-3 {
            continue;
        }
        let Some((offset, peak)) = correlate_valid(seg, probe)?
            .iter()
            .enumerate()
            .max_by(|left, right| left.1.total_cmp(right.1))
            .map(|(offset, v)| (offset, *v))
        else {
            continue;
        };
        let gain = peak / energy;
        let norm = seg[offset..offset + width]
            .iter()
            .map(|v| v * v)
            .sum::<f64>()
            .sqrt();
        let similarity = peak / (norm * probe_norm + 1e-12);
        let far = out.last().is_none_or(|last| lo + offset > last.0 + gap);
        if gain > 0.4 && similarity > 0.5 && far {
            out.push((lo + offset, gain));
        }
    }
    Ok(out)
}

fn shot(p: &[f64], tmpl: &[f64], n: usize) -> Result<(Vec<f64>, Vec<f64>)> {
    let mut kicked = kick(p, n);
    let len = tmpl.len().min(n);
    if kicked[..len]
        .iter()
        .zip(tmpl)
        .map(|(a, b)| a * b)
        .sum::<f64>()
        < 0.0
    {
        for v in &mut kicked {
            *v = -*v;
        }
    }
    let gain = rms(tmpl) / rms(&kicked[..len]);
    for v in &mut kicked {
        *v *= gain;
    }
    let block = samples(0.005)?;
    let half = block.to_f64().context("block")? / 2.0;
    let span = tmpl.len() / block * block;
    let block_rms =
        |signal: &[f64]| -> Vec<f64> { signal[..span].chunks_exact(block).map(rms).collect() };
    let correction: Vec<f64> = block_rms(tmpl)
        .iter()
        .zip(&block_rms(&kicked))
        .map(|(want, have)| (20.0 * ((want + 1e-9) / (have + 1e-9)).log10()).clamp(-12.0, 12.0))
        .collect();
    let correction = smooth3_edge(&correction);
    let xp: Vec<f64> = (0u32..)
        .take(correction.len())
        .map(|i| f64::from(i) * 2.0 * half + half)
        .collect();
    let mut curve: Vec<f64> = (0u32..)
        .take(n)
        .map(|i| interp(f64::from(i), &xp, &correction))
        .collect();
    let hold = curve[span - 1];
    curve[span..].iter_mut().for_each(|v| *v = hold);
    let shaped = kicked
        .iter()
        .zip(&curve)
        .map(|(v, g)| v * 10f64.powf(g / 20.0))
        .collect();
    Ok((shaped, correction))
}

fn track(
    p: &[f64],
    tmpl: &[f64],
    found: &[(usize, f64)],
    n: usize,
) -> Result<(Vec<Frame>, Vec<f64>)> {
    let (kicked, correction) = shot(p, tmpl, n)?;
    let mut mix = vec![0.0; n];
    for (i, (onset, gain)) in found.iter().enumerate() {
        let last = i + 1 == found.len();
        let room = if last {
            n - onset
        } else {
            found[i + 1].0.min(n) - onset
        };
        let len = if last { room } else { tmpl.len().min(room) };
        let mut seg = kicked[..len.min(kicked.len())].to_vec();
        let tail = 64.min(seg.len());
        let start = seg.len() - tail;
        let fade = (tail.max(2) - 1).to_f64().context("fade")?;
        for (v, j) in seg[start..].iter_mut().zip(0u32..) {
            *v *= 1.0 - f64::from(j) / fade;
        }
        for (j, v) in seg.iter().enumerate() {
            if let Some(slot) = mix.get_mut(onset + j) {
                *slot += gain * v;
            }
        }
    }
    Ok((dual(&mix), correction))
}

pub fn run_fit(
    song: &Song,
    stem: &str,
    section: &str,
    length: f64,
    hit_count: usize,
    max_iter: u64,
    seed: u64,
) -> Result<Value> {
    let x = song.stem(stem, None)?;
    let m = mono(song.section(&x, section)?);
    let (loss, p) = fit(&template(&m, length, hit_count)?, max_iter, seed)?;
    let params: Map<String, Value> = NAMES
        .iter()
        .zip(&p)
        .map(|(n, v)| ((*n).to_string(), json!(v)))
        .collect();
    Ok(json!({"stem": stem, "section": section, "loss": round(loss, 3), "params": params}))
}

pub fn run_track(
    song: &Song,
    stem: &str,
    section: &str,
    params: &Path,
    length: f64,
    hit_count: usize,
    out: &Path,
) -> Result<Value> {
    let frames = song.frames()?;
    let reference = song.stem(stem, Some(frames))?;
    let target = mono(&reference);
    let tmpl = template(&mono(song.section(&reference, section)?), length, hit_count)?;
    let fitted: Value = serde_json::from_str(&std::fs::read_to_string(params)?)?;
    let p: Vec<f64> = fitted["params"]
        .as_object()
        .context("params file has no params object")?
        .values()
        .filter_map(Value::as_f64)
        .collect();
    let found = hits(&target, &tmpl, song.beat / 4.0)?;
    let (rendered, correction) = track(&p, &tmpl, &found, frames)?;
    let (rendered, eq) = matched(&rendered, &reference, 3)?;
    write(out, &rendered, Format::Float)?;
    let (spec_db, env_db) = dist(&mono(&rendered), &target)?;
    let gains: Vec<f64> = found.iter().map(|f| f.1).collect();
    let spread: Vec<f64> = [5.0, 50.0, 95.0]
        .iter()
        .map(|q| Ok(round(percentile(&gains, *q)?, 3)))
        .collect::<Result<_>>()?;
    let correction: Vec<f64> = correction.iter().map(|v| round(*v, 1)).collect();
    Ok(json!({
        "stem": stem,
        "hits": found.len(),
        "hit_gain_p5_p50_p95": spread,
        "env_correction_db": correction,
        "eq_db": rounded(&eq)?,
        "spec_db": round(spec_db, 2),
        "env_db": round(env_db, 2),
    }))
}
