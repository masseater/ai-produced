use anyhow::{Context, Result};
use ndarray::ArrayView1;
use num_traits::ToPrimitive;
use sonara::core::pitch::estimate_tuning;
use sonara::feature::spectral::chroma_stft;

use crate::signal::{RATE, SR, hanning, moving_mean_same, rfft, rfftfreq};

#[must_use]
pub fn edges() -> Vec<f64> {
    (0..29)
        .map(|i| 40.0 * (f64::from(i) / 3.0).exp2())
        .collect()
}

#[must_use]
pub fn centers() -> Vec<f64> {
    edges().windows(2).map(|e| (e[0] * e[1]).sqrt()).collect()
}

pub fn band_power(m: &[f64]) -> Result<Vec<f64>> {
    let n = if m.len() >= 2 * 4096 { 4096 } else { 1024 };
    let window = hanning(n)?;
    let mut power = vec![0.0; n / 2 + 1];
    let mut frames = 0.0f64;
    let mut i = 0;
    while i + n < m.len() {
        let segment: Vec<f64> = m[i..i + n]
            .iter()
            .zip(&window)
            .map(|(v, w)| v * w)
            .collect();
        for (p, c) in power.iter_mut().zip(rfft(&segment, n)) {
            *p += c.norm_sqr();
        }
        frames += 1.0;
        i += n / 2;
    }
    let freqs = rfftfreq(n)?;
    Ok(edges()
        .windows(2)
        .map(|e| {
            power
                .iter()
                .zip(&freqs)
                .filter(|(_, f)| **f >= e[0] && **f < e[1])
                .map(|(p, _)| p)
                .sum::<f64>()
                / frames.max(1.0)
        })
        .collect())
}

pub fn ltas(m: &[f64]) -> Result<Vec<f64>> {
    let d: Vec<f64> = band_power(m)?
        .iter()
        .map(|p| 10.0 * (p + 1e-14).log10())
        .collect();
    let top = d.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    Ok(d.iter().map(|v| v - top).collect())
}

pub fn env(m: &[f64], hop: usize) -> Result<Vec<f64>> {
    let squares: Vec<f64> = m.iter().map(|v| v * v).collect();
    Ok(moving_mean_same(&squares, hop)?
        .iter()
        .step_by(hop)
        .map(|p| 20.0 * (p.sqrt() + 1e-9).log10())
        .collect())
}

#[must_use]
pub fn env_dist(ea: &[f64], eb: &[f64], floor: f64) -> f64 {
    let top = |e: &[f64]| e.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let (ta, tb) = (top(ea), top(eb));
    let (total, count) = ea
        .iter()
        .zip(eb)
        .map(|(a, b)| (a - ta, b - tb))
        .filter(|(a, b)| *b > floor || *a > floor)
        .map(|(a, b)| (a.clamp(floor, 0.0) - b.clamp(floor, 0.0)).abs())
        .fold((0.0, 0.0), |(total, count), d| (total + d, count + 1.0));
    total / count
}

pub fn dist(a: &[f64], b: &[f64]) -> Result<(f64, f64)> {
    let (la, lb) = (ltas(a)?, ltas(b)?);
    let (total, count) = la
        .iter()
        .zip(&lb)
        .filter(|(_, b)| **b > -60.0)
        .map(|(a, b)| (a - b).abs())
        .fold((0.0, 0.0), |(total, count), d| (total + d, count + 1.0));
    Ok((total / count, env_dist(&env(a, 441)?, &env(b, 441)?, -50.0)))
}

pub fn chroma(m: &[f64]) -> Result<Vec<f64>> {
    let y: Vec<f32> = m
        .iter()
        .map(|v| v.to_f32().context("sample"))
        .collect::<Result<_>>()?;
    let view = ArrayView1::from(&y);
    let tuning = estimate_tuning(Some(view), SR, None, None, None).unwrap_or(0.0);
    let rate = RATE.to_f32().context("sample rate")?;
    let Ok(c) = chroma_stft(Some(view), None, rate, 4096, 1024, 12, tuning) else {
        return Ok(vec![0.0; 12]);
    };
    let v: Vec<f64> = c
        .rows()
        .into_iter()
        .map(|row| f64::from(row.mean().unwrap_or(0.0)))
        .collect();
    let norm = v.iter().map(|x| x * x).sum::<f64>().sqrt();
    Ok(v.iter().map(|x| x / (norm + 1e-12)).collect())
}

pub fn dist3(a: &[f64], b: &[f64], chroma_b: &[f64]) -> Result<(f64, f64, f64)> {
    let (s, e) = dist(a, b)?;
    let dot: f64 = chroma(a)?.iter().zip(chroma_b).map(|(x, y)| x * y).sum();
    Ok((s, e, 10.0 * (1.0 - dot)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn third_octave_bands_span_40_hz_to_16_khz() {
        let e = edges();
        assert_eq!(e.len(), 29);
        assert!((e[0] - 40.0).abs() < 1e-12);
        assert!((e[27] - 40.0 * 512.0).abs() < 1e-9);
    }

    #[test]
    fn identical_signals_have_zero_distance() {
        let x: Vec<f64> = (0..44_100u32)
            .map(|i| (f64::from(i) * 0.05).sin() * (f64::from(i) / 44_100.0))
            .collect();
        let (s, e) = dist(&x, &x).unwrap();
        assert!(s.abs() < 1e-12 && e.abs() < 1e-12);
    }

    #[test]
    fn chroma_of_a440_peaks_at_a() {
        let x: Vec<f64> = (0..44_100u32)
            .map(|i| (2.0 * std::f64::consts::PI * 440.0 * f64::from(i) / RATE).sin())
            .collect();
        let c = chroma(&x).unwrap();
        let top = c
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(b.1))
            .unwrap()
            .0;
        assert_eq!(top, 9);
    }
}
