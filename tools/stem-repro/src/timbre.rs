use std::f64::consts::PI;
use std::path::Path;

use anyhow::{Context, Result, bail};
use num_traits::ToPrimitive;
use rustfft::FftPlanner;
use rustfft::num_complex::Complex;
use serde_json::{Map, Value, json};

use crate::features::dist;
use crate::notes::{Note, read};
use crate::signal::{
    RATE, hamming, hann_periodic, load, mean, mean_square, mono, percentile, rfft, rfftfreq, round,
};
use crate::song::Song;

const HOP: usize = 256;
const WINDOWS: [(f64, f64); 4] = [(0.0, 30.0), (30.0, 80.0), (80.0, 150.0), (150.0, 300.0)];

fn hz(pitch: u8) -> f64 {
    440.0 * ((f64::from(pitch) - 69.0) / 12.0).exp2()
}

fn hop_frames(seconds: f64) -> Result<f64> {
    Ok(seconds * RATE / HOP.to_f64().context("hop")?)
}

fn frame_index(seconds: f64) -> Result<usize> {
    hop_frames(seconds)?
        .trunc()
        .max(0.0)
        .to_usize()
        .with_context(|| format!("bad time {seconds} s"))
}

fn f0_frames(notes: &[Note], t0: f64, n: usize, voices: usize) -> Result<Vec<Vec<f64>>> {
    let mut f = vec![vec![0.0; voices]; n];
    for note in notes {
        let first = frame_index(note.start - t0)?;
        let last = (hop_frames(note.end - t0)?.trunc() + 1.0)
            .max(0.0)
            .to_usize()
            .context("note end")?
            .min(n);
        for row in f.iter_mut().take(last).skip(first) {
            let mut values: Vec<f64> = row.iter().copied().filter(|v| *v > 0.0).collect();
            values.push(hz(note.pitch));
            values.sort_by(f64::total_cmp);
            values.resize(voices, 0.0);
            *row = values;
        }
    }
    Ok(f)
}

fn unwrap(phase: &mut [f64]) {
    let mut offset = 0.0;
    for i in 1..phase.len() {
        let raw = phase[i] + offset;
        let d = raw - phase[i - 1];
        let wrapped = (d + PI).rem_euclid(2.0 * PI) - PI;
        let wrapped = if wrapped <= -PI && d > 0.0 {
            PI
        } else {
            wrapped
        };
        let correction = wrapped - d;
        if d.abs() >= PI {
            offset += correction;
        }
        phase[i] += offset;
    }
}

fn dft_analysis(x: &[f64], w: &[f64], n: usize) -> (Vec<f64>, Vec<f64>) {
    let m = w.len();
    let (h1, h2) = (m.div_ceil(2), m / 2);
    let total: f64 = w.iter().sum();
    let xw: Vec<f64> = x.iter().zip(w).map(|(a, b)| a * b / total).collect();
    let mut buffer = vec![0.0; n];
    buffer[..h1].copy_from_slice(&xw[h2..]);
    buffer[n - h2..].copy_from_slice(&xw[..h2]);
    let spectrum = rfft(&buffer, n);
    let magnitude: Vec<f64> = spectrum
        .iter()
        .map(|c| 20.0 * c.norm().max(f64::EPSILON).log10())
        .collect();
    let mut phase: Vec<f64> = spectrum
        .iter()
        .map(|c| {
            let re = if c.re.abs() < 1e-14 { 0.0 } else { c.re };
            let im = if c.im.abs() < 1e-14 { 0.0 } else { c.im };
            im.atan2(re)
        })
        .collect();
    unwrap(&mut phase);
    (magnitude, phase)
}

fn peak_detection(mx: &[f64], threshold: f64) -> Vec<usize> {
    (1..mx.len() - 1)
        .filter(|k| {
            let v = mx[*k];
            v > threshold && v > mx[k + 1] && v > mx[k - 1] && v != 0.0
        })
        .collect()
}

fn peak_interp(
    mx: &[f64],
    px: &[f64],
    locations: &[usize],
) -> Result<(Vec<f64>, Vec<f64>, Vec<f64>)> {
    let mut loc = Vec::with_capacity(locations.len());
    let mut mag = Vec::with_capacity(locations.len());
    let mut phase = Vec::with_capacity(locations.len());
    for &peak in locations {
        let (left, mid, right) = (mx[peak - 1], mx[peak], mx[peak + 1]);
        let bin = peak.to_f64().context("peak bin")?;
        let at = bin + 0.5 * (left - right) / (left - 2.0 * mid + right);
        loc.push(at);
        mag.push(mid - 0.25 * (left - right) * (at - bin));
        let lo = at.floor().to_usize().context("peak position")?;
        let fraction = at - at.floor();
        phase.push(px[lo] + fraction * (px[(lo + 1).min(px.len() - 1)] - px[lo]));
    }
    Ok((loc, mag, phase))
}

struct Harmonics {
    freq: Vec<f64>,
    mag: Vec<f64>,
    phase: Vec<f64>,
}

fn harmonic_detection(
    pfreq: &[f64],
    pmag: &[f64],
    pphase: &[f64],
    f0: f64,
    count: usize,
    previous: &[f64],
) -> Harmonics {
    let mut out = Harmonics {
        freq: vec![0.0; count],
        mag: vec![-100.0; count],
        phase: vec![0.0; count],
    };
    for (hi, number) in (0..count).zip(1u32..) {
        let target = f0 * f64::from(number);
        if target >= RATE / 2.0 {
            break;
        }
        let pei = pfreq
            .iter()
            .enumerate()
            .min_by(|a, b| (a.1 - target).abs().total_cmp(&(b.1 - target).abs()))
            .map_or(0, |(i, _)| i);
        let dev1 = (pfreq[pei] - target).abs();
        let dev2 = if previous[hi] > 0.0 {
            (pfreq[pei] - previous[hi]).abs()
        } else {
            RATE
        };
        let threshold = f0 / 3.0 + 0.01 * pfreq[pei];
        if dev1 < threshold || dev2 < threshold {
            out.freq[hi] = pfreq[pei];
            out.mag[hi] = pmag[pei];
            out.phase[hi] = pphase[pei];
        }
    }
    out
}

fn blackman_harris(n: usize) -> Result<Vec<f64>> {
    let a = [0.35875, 0.48829, 0.14128, 0.01168];
    let last = (n - 1).to_f64().context("window length")?;
    Ok((0u32..)
        .take(n)
        .map(|i| {
            let f = -PI + 2.0 * PI * f64::from(i) / last;
            a[0] + a[1] * f.cos() + a[2] * (2.0 * f).cos() + a[3] * (3.0 * f).cos()
        })
        .collect())
}

fn bh_lobe(x: f64) -> f64 {
    let size = 512.0;
    let freq = x * PI * 2.0 / size;
    let df = 2.0 * PI / size;
    let sinc = |v: f64| {
        let ratio = (size * v / 2.0).sin() / (v / 2.0).sin();
        if ratio.is_nan() { size } else { ratio }
    };
    let weights = [0.35875, 0.48829, 0.14128, 0.01168];
    let lobe: f64 = weights
        .iter()
        .zip(0u32..)
        .map(|(weight, k)| {
            weight / 2.0 * (sinc(freq - df * f64::from(k)) + sinc(freq + df * f64::from(k)))
        })
        .sum();
    lobe / size / weights[0]
}

fn spectral_sines(
    locations: &[f64],
    mags: &[f64],
    phases: &[f64],
    n: usize,
) -> Result<Vec<Complex<f64>>> {
    let mut y = vec![Complex::new(0.0, 0.0); n];
    let half = n / 2;
    let top = (half - 1).to_f64().context("FFT size")?;
    for ((&loc, &mag), &phase) in locations.iter().zip(mags).zip(phases) {
        if loc == 0.0 || loc > top {
            continue;
        }
        let centre = loc.round_ties_even();
        let remainder = centre - loc;
        let amplitude = 10f64.powf(mag / 20.0);
        for m in 0..9 {
            let lobe = bh_lobe(remainder - 4.0 + f64::from(m)) * amplitude;
            let b = centre - 4.0 + f64::from(m);
            let forward = Complex::from_polar(lobe, phase);
            let backward = Complex::from_polar(lobe, -phase);
            let index = b.abs().to_usize().context("lobe bin")?;
            if b < 0.0 || index > half {
                y[index] += backward;
            } else if index == 0 || index == half {
                y[index] += forward + backward;
            } else {
                y[index] += forward;
            }
        }
        for k in half + 1..n {
            y[k] = y[n - k].conj();
        }
    }
    Ok(y)
}

fn sine_subtraction(x: &[f64], n: usize, h: usize, harmonics: &[Harmonics]) -> Result<Vec<f64>> {
    let half = n / 2;
    let size = n.to_f64().context("FFT size")?;
    let mut padded = vec![0.0; half];
    padded.extend_from_slice(x);
    padded.extend(std::iter::repeat_n(0.0, half));
    let bh = blackman_harris(n)?;
    let total: f64 = bh.iter().sum();
    let w: Vec<f64> = bh.iter().map(|v| v / total).collect();
    let mut sw = vec![0.0; n];
    let width = (2 * h).to_f64().context("hop")?;
    for (k, i) in (half - h..half + h).enumerate() {
        let step = (k.min(2 * h - 1 - k) + 1).to_f64().context("hop")?;
        sw[i] = (2.0 * step - 1.0) / width / w[i];
    }
    let mut planner = FftPlanner::<f64>::new();
    let forward = planner.plan_fft_forward(n);
    let inverse = planner.plan_fft_inverse(n);
    let mut residual = vec![0.0; padded.len()];
    let mut pin = 0;
    for frame in harmonics {
        if pin + n > padded.len() {
            break;
        }
        let mut buffer: Vec<Complex<f64>> = (0..n)
            .map(|i| Complex::new(padded[pin + (i + half) % n] * w[(i + half) % n], 0.0))
            .collect();
        forward.process(&mut buffer);
        let locations: Vec<f64> = frame.freq.iter().map(|f| size * f / RATE).collect();
        let sines = spectral_sines(&locations, &frame.mag, &frame.phase, n)?;
        for (b, s) in buffer.iter_mut().zip(&sines) {
            *b -= s;
        }
        inverse.process(&mut buffer);
        for (i, weight) in sw.iter().enumerate() {
            residual[pin + i] += buffer[(i + half) % n].re / size * weight;
        }
        pin += h;
    }
    Ok(residual[half..residual.len() - half].to_vec())
}

struct Split {
    harmonic: Vec<f64>,
    residual: Vec<f64>,
    first_voice: Vec<Vec<f64>>,
    window: usize,
}

fn split(x: &[f64], notes: &[Note], t0: f64) -> Result<Split> {
    let (voices, count) = (4, 40);
    let lowest = notes
        .iter()
        .map(|note| hz(note.pitch))
        .fold(f64::INFINITY, f64::min);
    let m = (4.0 * RATE / lowest).to_usize().context("window length")? | 1;
    let nfft = (2 * m).next_power_of_two();
    let bin = RATE / nfft.to_f64().context("FFT size")?;
    let window = hamming(m)?;
    let mut padded = vec![0.0; m / 2];
    padded.extend_from_slice(x);
    padded.extend(std::iter::repeat_n(0.0, m.div_ceil(2)));
    let frames = (padded.len() - m) / HOP;
    let f0 = f0_frames(notes, t0, frames + 1, voices)?;
    let mut previous = vec![vec![0.0; count]; voices];
    let mut rows: Vec<Harmonics> = Vec::with_capacity(frames);
    let mut first_voice = Vec::with_capacity(frames);
    for (i, targets) in f0.iter().enumerate().take(frames) {
        let center = m.div_ceil(2) + i * HOP;
        let (mx, px) = dft_analysis(
            &padded[center - m.div_ceil(2)..center + m / 2],
            &window,
            nfft,
        );
        let peaks = peak_detection(&mx, -100.0);
        let (loc, mag, phase) = peak_interp(&mx, &px, &peaks)?;
        let pfreq: Vec<f64> = loc.iter().map(|at| at * bin).collect();
        let mut row = Harmonics {
            freq: Vec::with_capacity(count * voices),
            mag: Vec::with_capacity(count * voices),
            phase: Vec::with_capacity(count * voices),
        };
        for v in 0..voices {
            let found = if targets[v] <= 0.0 || pfreq.is_empty() {
                previous[v] = vec![0.0; count];
                Harmonics {
                    freq: vec![0.0; count],
                    mag: vec![-200.0; count],
                    phase: vec![0.0; count],
                }
            } else {
                let detected =
                    harmonic_detection(&pfreq, &mag, &phase, targets[v], count, &previous[v]);
                previous[v].clone_from(&detected.freq);
                detected
            };
            if v == 0 {
                first_voice.push(found.mag.clone());
            }
            row.freq.extend(found.freq);
            row.mag.extend(found.mag);
            row.phase.extend(found.phase);
        }
        rows.push(row);
    }
    let mut residual = sine_subtraction(x, 512, HOP, &rows)?;
    residual.resize(x.len(), 0.0);
    Ok(Split {
        harmonic: x.iter().zip(&residual).map(|(a, b)| a - b).collect(),
        residual,
        first_voice,
        window: m,
    })
}

fn magnitude_frames(x: &[f64], n: usize) -> Result<Vec<Vec<f64>>> {
    let window = hann_periodic(n)?;
    let mut padded = vec![0.0; n / 2];
    padded.extend_from_slice(x);
    padded.extend(std::iter::repeat_n(0.0, n / 2));
    Ok((0..=(padded.len() - n) / HOP)
        .map(|k| {
            let segment: Vec<f64> = padded[k * HOP..k * HOP + n]
                .iter()
                .zip(&window)
                .map(|(v, w)| v * w)
                .collect();
            rfft(&segment, n).iter().map(|c| c.norm()).collect()
        })
        .collect())
}

fn descriptors(x: &[f64], onsets: &[f64]) -> Result<Vec<(&'static str, Vec<f64>)>> {
    let n = 1024;
    let bins = 60;
    let s = magnitude_frames(x, n)?;
    let freqs = rfftfreq(n)?;
    let size = (n / 2 + 1).to_f64().context("spectrum size")?;
    let centroid: Vec<f64> = s
        .iter()
        .map(|f| {
            let total: f64 = f.iter().sum();
            f.iter().zip(&freqs).map(|(m, q)| m * q).sum::<f64>() / total
        })
        .collect();
    let rolloff: Vec<f64> = s
        .iter()
        .map(|f| {
            let threshold = 0.85 * f.iter().sum::<f64>();
            let mut acc = 0.0;
            f.iter()
                .zip(&freqs)
                .find(|(m, _)| {
                    acc += **m;
                    acc >= threshold
                })
                .map_or(0.0, |(_, q)| *q)
        })
        .collect();
    let flatness: Vec<f64> = s
        .iter()
        .map(|f| {
            let p: Vec<f64> = f.iter().map(|m| (m * m).max(1e-10)).collect();
            let log_mean = p.iter().map(|v| v.ln()).sum::<f64>() / size;
            log_mean.exp() / (p.iter().sum::<f64>() / size)
        })
        .collect();
    let db: Vec<Vec<f64>> = {
        let raw: Vec<Vec<f64>> = s
            .iter()
            .map(|f| f.iter().map(|m| 20.0 * m.max(1e-5).log10()).collect())
            .collect();
        let top = raw
            .iter()
            .flatten()
            .copied()
            .fold(f64::NEG_INFINITY, f64::max);
        raw.iter()
            .map(|f| f.iter().map(|v| v.max(top - 80.0)).collect())
            .collect()
    };
    let pad = 1 + 2048 / (2 * HOP);
    let mut flux = vec![0.0; pad];
    flux.extend(db.windows(2).map(|w| {
        w[1].iter()
            .zip(&w[0])
            .map(|(b, a)| (b - a).max(0.0))
            .sum::<f64>()
            / size
    }));
    flux.truncate(s.len());
    let starts: Vec<usize> = onsets
        .iter()
        .map(|t| frame_index(*t))
        .collect::<Result<_>>()?;
    let median = |v: &[f64]| -> Result<Vec<f64>> {
        let rows: Vec<&[f64]> = starts
            .iter()
            .filter(|i| *i + bins <= v.len())
            .map(|i| &v[*i..*i + bins])
            .collect();
        (0..bins)
            .map(|j| {
                let column: Vec<f64> = rows.iter().map(|r| r[j]).collect();
                percentile(&column, 50.0)
            })
            .collect()
    };
    Ok(vec![
        ("centroid", median(&centroid)?),
        ("rolloff", median(&rolloff)?),
        ("flatness", median(&flatness)?),
        ("flux", median(&flux)?),
    ])
}

fn harmonic_profile(hm: &[Vec<f64>], onsets: &[f64]) -> Result<Vec<Vec<f64>>> {
    let (bins, n) = (60, 12);
    let mut rows: Vec<usize> = Vec::new();
    for t in onsets {
        let i = frame_index(*t)?;
        if i + bins <= hm.len() {
            rows.push(i);
        }
    }
    let p: Vec<Vec<f64>> = (0..bins)
        .map(|b| {
            (0..n)
                .map(|h| {
                    let power = mean(rows.iter().map(|i| 10f64.powf(hm[i + b][h] / 10.0)))?;
                    Ok(10.0 * (power + 1e-20).log10())
                })
                .collect()
        })
        .collect::<Result<_>>()?;
    let top = p
        .iter()
        .flatten()
        .copied()
        .fold(f64::NEG_INFINITY, f64::max);
    Ok(p.iter()
        .map(|r| r.iter().map(|v| (v - top).max(-60.0)).collect())
        .collect())
}

fn window_rows(rows: usize, a: f64, b: f64) -> Result<std::ops::Range<usize>> {
    let (start, end) = (frame_index(a / 1000.0)?, frame_index(b / 1000.0)?);
    Ok(start.min(rows)..(end + 1).min(rows))
}

fn windowed_columns(rows: &[Vec<f64>], digits: i32) -> Result<Value> {
    let mut out = Map::new();
    for (a, b) in WINDOWS {
        let range = window_rows(rows.len(), a, b)?;
        let width = rows.first().map_or(0, Vec::len);
        let means: Vec<f64> = (0..width)
            .map(|c| {
                Ok(round(
                    mean(rows[range.clone()].iter().map(|r| r[c]))?,
                    digits,
                ))
            })
            .collect::<Result<_>>()?;
        out.insert(format!("{a}-{b}ms"), json!(means));
    }
    Ok(Value::Object(out))
}

fn compare(reference: &[f64], repro: &[f64], notes: &[Note], t0: f64) -> Result<Value> {
    let gain = mean_square(reference).sqrt() / (mean_square(repro).sqrt() + 1e-12);
    let repro: Vec<f64> = repro.iter().map(|v| v * gain).collect();
    let r = split(reference, notes, t0)?;
    let y = split(&repro, notes, t0)?;
    let score = |a: &[f64]| -> Result<Value> {
        let (s, e) = dist(a, reference)?;
        Ok(json!({"spec_db": round(s, 2), "env_db": round(e, 2)}))
    };
    let share =
        |res: &[f64], x: &[f64]| round(10.0 * (mean_square(res) / mean_square(x)).log10(), 1);
    let mut ons: Vec<f64> = notes.iter().map(|n| n.start - t0).collect();
    ons.sort_by(f64::total_cmp);
    ons.dedup();
    let (dr, dy) = (descriptors(reference, &ons)?, descriptors(&repro, &ons)?);
    let mut described = Map::new();
    for ((name, a), (_, b)) in dr.iter().zip(&dy) {
        let rows: Vec<Vec<f64>> = a.iter().zip(b).map(|(x, y)| vec![*x, *y]).collect();
        described.insert((*name).to_string(), windowed_columns(&rows, 3)?);
    }
    let (pr, py) = (
        harmonic_profile(&r.first_voice, &ons)?,
        harmonic_profile(&y.first_voice, &ons)?,
    );
    let diff: Vec<Vec<f64>> = py
        .iter()
        .zip(&pr)
        .map(|(a, b)| a.iter().zip(b).map(|(x, y)| x - y).collect())
        .collect();
    let swapped_harmonics: Vec<f64> = r
        .harmonic
        .iter()
        .zip(&y.residual)
        .map(|(a, b)| a + b)
        .collect();
    let swapped_residual: Vec<f64> = y
        .harmonic
        .iter()
        .zip(&r.residual)
        .map(|(a, b)| a + b)
        .collect();
    Ok(json!({
        "window_ms": round(r.window.to_f64().context("window")? / RATE * 1000.0, 1),
        "residual_share_db": {"ref": share(&r.residual, reference), "repro": share(&y.residual, &repro)},
        "swap": {
            "repro": score(&repro)?,
            "ref_harmonics": score(&swapped_harmonics)?,
            "ref_residual": score(&swapped_residual)?,
        },
        "descriptors": described,
        "harmonics_ref_db": windowed_columns(&pr, 1)?,
        "harmonics_diff_db": windowed_columns(&diff, 1)?,
    }))
}

pub fn run(
    song: &Song,
    stem: &str,
    repro: &Path,
    bars: [f64; 2],
    part: Option<&str>,
    midi: Option<&Path>,
) -> Result<Value> {
    let [b0, b1] = bars;
    let t0 = b0 * song.bar;
    let all = match (midi, part) {
        (Some(m), _) => read(m, None)?,
        (None, Some(p)) => song.part_notes(p)?,
        (None, None) => bail!("timbre needs --midi or --part"),
    };
    let notes = song.notes_in(&all, b0, b1);
    if notes.is_empty() {
        bail!("no notes between bars {b0} and {b1}");
    }
    let n = song.frames()?;
    let reference = mono(song.bars(&song.stem(stem, Some(n))?, b0, b1));
    let repro = mono(song.bars(&load(repro, Some(n))?, b0, b1));
    let mut out = Map::new();
    out.insert("stem".into(), json!(stem));
    out.insert("bars".into(), json!(bars));
    out.insert("notes".into(), json!(notes.len()));
    if let Value::Object(rest) = compare(&reference, &repro, &notes, t0)? {
        out.extend(rest);
    }
    Ok(Value::Object(out))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sine_subtraction_removes_a_tracked_tone() {
        let f = 220.0;
        let x: Vec<f64> = (0..22_050)
            .map(|i| 0.5 * (2.0 * PI * f * f64::from(i) / RATE).sin())
            .collect();
        let notes = [Note {
            start: 0.0,
            end: 0.5,
            pitch: 57,
            velocity: 100,
        }];
        let s = split(&x, &notes, 0.0).unwrap();
        let middle = 4410..17_640;
        let residual = mean_square(&s.residual[middle.clone()]);
        let total = mean_square(&x[middle]);
        assert!(
            10.0 * (residual / total).log10() < -20.0,
            "{residual} {total}"
        );
    }
}
