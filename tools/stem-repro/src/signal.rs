use std::f64::consts::PI;
use std::path::Path;

use anyhow::{Context, Result, bail};
use ebur128::{EbuR128, Mode};
use num_traits::ToPrimitive;
use realfft::RealFftPlanner;
use rustfft::FftPlanner;
use rustfft::num_complex::Complex;
use serde::Serialize;

pub use audio::{mean, percentile, round};

pub const SR: u32 = 44_100;
pub const RATE: f64 = 44_100.0;

pub type Frame = [f64; 2];

pub fn load(path: &Path, frames: Option<usize>) -> Result<Vec<Frame>> {
    let decoded = audio::decode(path)?;
    if decoded.rate != SR {
        bail!(
            "{}: sample rate {}, expected {SR}",
            path.display(),
            decoded.rate
        );
    }
    let channels = decoded.channels;
    let mut out: Vec<Frame> = decoded
        .samples
        .chunks_exact(channels)
        .map(|frame| {
            let left = f64::from(frame[0]);
            let right = frame.get(1).map_or(left, |r| f64::from(*r));
            [left, right]
        })
        .collect();
    if let Some(n) = frames {
        out.resize(n, [0.0; 2]);
    }
    Ok(out)
}

#[derive(Clone, Copy)]
pub enum Format {
    Float,
    Pcm24,
}

pub fn write(path: &Path, x: &[Frame], format: Format) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let (bits, sample_format) = match format {
        Format::Float => (32, hound::SampleFormat::Float),
        Format::Pcm24 => (24, hound::SampleFormat::Int),
    };
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: SR,
        bits_per_sample: bits,
        sample_format,
    };
    let mut writer = hound::WavWriter::create(path, spec)?;
    for value in x.iter().flatten() {
        match format {
            Format::Float => writer.write_sample(value.to_f32().context("sample")?)?,
            Format::Pcm24 => {
                let scaled = (value.clamp(-1.0, 1.0) * 8_388_607.0)
                    .round()
                    .to_i32()
                    .context("sample")?;
                writer.write_sample(scaled)?;
            }
        }
    }
    writer.finalize()?;
    Ok(())
}

#[must_use]
pub fn mono(x: &[Frame]) -> Vec<f64> {
    x.iter().map(|[l, r]| f64::midpoint(*l, *r)).collect()
}

#[must_use]
pub fn mid_side(x: &[Frame]) -> (Vec<f64>, Vec<f64>) {
    x.iter()
        .map(|[l, r]| (f64::midpoint(*l, *r), (l - r) / 2.0))
        .unzip()
}

#[must_use]
pub fn from_mid_side(mid: &[f64], side: &[f64]) -> Vec<Frame> {
    mid.iter().zip(side).map(|(m, s)| [m + s, m - s]).collect()
}

#[must_use]
pub fn dual(m: &[f64]) -> Vec<Frame> {
    m.iter().map(|v| [*v, *v]).collect()
}

#[must_use]
pub fn scale(x: &[Frame], gain: f64) -> Vec<Frame> {
    x.iter().map(|[l, r]| [l * gain, r * gain]).collect()
}

#[must_use]
pub fn apply_gain(x: &[Frame], gain: &[f64]) -> Vec<Frame> {
    x.iter()
        .zip(gain)
        .map(|([l, r], g)| [l * g, r * g])
        .collect()
}

#[must_use]
pub fn sum(stems: &[Vec<Frame>]) -> Vec<Frame> {
    let n = stems.iter().map(Vec::len).max().unwrap_or(0);
    let mut out = vec![[0.0; 2]; n];
    for stem in stems {
        for (o, [l, r]) in out.iter_mut().zip(stem) {
            o[0] += l;
            o[1] += r;
        }
    }
    out
}

#[must_use]
pub fn mean_square(x: &[f64]) -> f64 {
    let (total, count) = x
        .iter()
        .fold((0.0, 0.0), |(total, count), v| (total + v * v, count + 1.0));
    total / count
}

#[must_use]
pub fn rms(x: &[f64]) -> f64 {
    mean_square(x).sqrt()
}

#[must_use]
pub fn peak(x: &[Frame]) -> f64 {
    x.iter().flatten().fold(0.0, |a, v| a.max(v.abs()))
}

#[must_use]
pub fn db(value: f64) -> f64 {
    20.0 * value.log10()
}

#[must_use]
pub fn lufs(x: &[Frame]) -> Option<f64> {
    let mut meter = EbuR128::new(2, SR, Mode::I).ok()?;
    let interleaved: Vec<f64> = x.iter().flatten().copied().collect();
    meter.add_frames_f64(&interleaved).ok()?;
    let value = meter.loudness_global().ok()?;
    value.is_finite().then(|| round(value, 1))
}

#[must_use]
pub fn side_mid_db(x: &[Frame]) -> f64 {
    let (m, s) = mid_side(x);
    round(db(rms(&s) / (rms(&m) + 1e-12) + 1e-9), 1)
}

#[must_use]
pub fn lr_db(x: &[Frame]) -> f64 {
    let left: Vec<f64> = x.iter().map(|f| f[0]).collect();
    let right: Vec<f64> = x.iter().map(|f| f[1]).collect();
    round(db((rms(&left) + 1e-12) / (rms(&right) + 1e-12)), 1)
}

#[derive(Serialize, Clone)]
pub struct Stats {
    pub lufs: Option<f64>,
    pub crest: f64,
    pub sm: f64,
}

#[must_use]
pub fn stats(x: &[Frame]) -> Stats {
    let (m, _) = mid_side(x);
    let loud: Vec<f64> = m.iter().copied().filter(|v| v.abs() > 1e-4).collect();
    let active = if loud.is_empty() { &m } else { &loud };
    Stats {
        lufs: lufs(x),
        crest: round(db(peak(x) / (rms(active) + 1e-12)), 1),
        sm: side_mid_db(x),
    }
}

pub fn hanning(n: usize) -> Result<Vec<f64>> {
    cosine_window(n, 0.5, n.saturating_sub(1))
}

pub fn hamming(n: usize) -> Result<Vec<f64>> {
    cosine_window(n, 0.54, n.saturating_sub(1))
}

pub fn hann_periodic(n: usize) -> Result<Vec<f64>> {
    cosine_window(n, 0.5, n)
}

fn cosine_window(n: usize, a: f64, period: usize) -> Result<Vec<f64>> {
    if n == 1 {
        return Ok(vec![1.0]);
    }
    let period = period.to_f64().context("window length")?;
    Ok((0u32..)
        .take(n)
        .map(|k| a - (1.0 - a) * (2.0 * PI * f64::from(k) / period).cos())
        .collect())
}

#[must_use]
pub fn rfft(x: &[f64], n: usize) -> Vec<Complex<f64>> {
    let plan = RealFftPlanner::<f64>::new().plan_fft_forward(n);
    let mut input = vec![0.0; n];
    let take = x.len().min(n);
    input[..take].copy_from_slice(&x[..take]);
    let mut output = plan.make_output_vec();
    plan.process(&mut input, &mut output)
        .expect("buffer sizes come from the plan");
    output
}

pub fn irfft(spectrum: &[Complex<f64>], n: usize) -> Result<Vec<f64>> {
    let size = n.to_f64().context("FFT size")?;
    let plan = RealFftPlanner::<f64>::new().plan_fft_inverse(n);
    let mut input = plan.make_input_vec();
    let take = spectrum.len().min(input.len());
    input[..take].copy_from_slice(&spectrum[..take]);
    input[0].im = 0.0;
    if n.is_multiple_of(2) {
        let last = input.len() - 1;
        input[last].im = 0.0;
    }
    let mut output = plan.make_output_vec();
    plan.process(&mut input, &mut output)
        .expect("buffer sizes come from the plan");
    Ok(output.iter().map(|v| v / size).collect())
}

fn fft(x: &mut [Complex<f64>], inverse: bool) -> Result<()> {
    let mut planner = FftPlanner::new();
    let plan = if inverse {
        planner.plan_fft_inverse(x.len())
    } else {
        planner.plan_fft_forward(x.len())
    };
    plan.process(x);
    if inverse {
        let n = x.len().to_f64().context("FFT size")?;
        for v in x.iter_mut() {
            *v /= n;
        }
    }
    Ok(())
}

pub fn rfftfreq(n: usize) -> Result<Vec<f64>> {
    let size = n.to_f64().context("FFT size")?;
    Ok((0u32..)
        .take(n / 2 + 1)
        .map(|k| f64::from(k) * RATE / size)
        .collect())
}

pub fn fft_convolve(a: &[f64], b: &[f64]) -> Result<Vec<f64>> {
    if a.is_empty() || b.is_empty() {
        return Ok(Vec::new());
    }
    let full = a.len() + b.len() - 1;
    let n = full.next_power_of_two();
    let fa = rfft(a, n);
    let fb = rfft(b, n);
    let product: Vec<Complex<f64>> = fa.iter().zip(&fb).map(|(x, y)| x * y).collect();
    let mut out = irfft(&product, n)?;
    out.truncate(full);
    Ok(out)
}

pub fn correlate_full(a: &[f64], v: &[f64]) -> Result<Vec<f64>> {
    let reversed: Vec<f64> = v.iter().rev().copied().collect();
    fft_convolve(a, &reversed)
}

pub fn correlate_valid(a: &[f64], v: &[f64]) -> Result<Vec<f64>> {
    if a.len() < v.len() {
        return Ok(Vec::new());
    }
    Ok(correlate_full(a, v)?[v.len() - 1..a.len()].to_vec())
}

pub fn moving_mean_same(x: &[f64], k: usize) -> Result<Vec<f64>> {
    let width = k.to_f64().context("window length")?;
    let mut prefix = vec![0.0; x.len() + 1];
    for (i, v) in x.iter().enumerate() {
        prefix[i + 1] = prefix[i] + v;
    }
    let offset = (k - 1) / 2;
    Ok((0..x.len())
        .map(|t| {
            let j = t + offset;
            let lo = (j + 1).saturating_sub(k);
            let hi = (j + 1).min(x.len());
            (prefix[hi] - prefix[lo]) / width
        })
        .collect())
}

pub fn box_smooth_edge(x: &[f64], half: usize) -> Result<Vec<f64>> {
    let Some(last) = x.len().checked_sub(1) else {
        return Ok(Vec::new());
    };
    let at = |i: usize| x[i.min(last)];
    let width = (2 * half + 1).to_f64().context("window length")?;
    let mut acc = x[0] * half.to_f64().context("window length")? + (0..=half).map(at).sum::<f64>();
    let mut out = Vec::with_capacity(x.len());
    for i in 0..x.len() {
        out.push(acc / width);
        acc += at(i + half + 1) - x[i.saturating_sub(half)];
    }
    Ok(out)
}

#[must_use]
pub fn smooth3_edge(x: &[f64]) -> Vec<f64> {
    let last = x.len().saturating_sub(1);
    (0..x.len())
        .map(|i| 0.25 * x[i.saturating_sub(1)] + 0.5 * x[i] + 0.25 * x[(i + 1).min(last)])
        .collect()
}

#[must_use]
pub fn interp(x: f64, xp: &[f64], fp: &[f64]) -> f64 {
    if x <= xp[0] {
        return fp[0];
    }
    let last = xp.len() - 1;
    if x >= xp[last] {
        return fp[last];
    }
    let i = xp.partition_point(|v| *v <= x) - 1;
    let t = (x - xp[i]) / (xp[i + 1] - xp[i]);
    fp[i] + t * (fp[i + 1] - fp[i])
}

pub type Sos = [f64; 6];

#[must_use]
pub fn butter(order: i32, cutoff: f64, highpass: bool) -> Vec<Sos> {
    let fs2 = 4.0;
    let warped = fs2 * (PI * cutoff / 2.0).tan();
    let analog: Vec<Complex<f64>> = (0..order)
        .map(|k| {
            let m = -f64::from(order) + 1.0 + 2.0 * f64::from(k);
            -Complex::from_polar(1.0, PI * m / (2.0 * f64::from(order)))
        })
        .collect();
    let (poles, zero, mut gain): (Vec<Complex<f64>>, f64, f64) = if highpass {
        let poles: Vec<Complex<f64>> = analog.iter().map(|p| warped / p).collect();
        let product = analog.iter().fold(Complex::new(1.0, 0.0), |a, p| a * -p);
        (poles, 1.0, (Complex::new(1.0, 0.0) / product).re)
    } else {
        (
            analog.iter().map(|p| p * warped).collect(),
            -1.0,
            warped.powi(order),
        )
    };
    let analog_zeros = if highpass { order } else { 0 };
    let digital: Vec<Complex<f64>> = poles.iter().map(|p| (fs2 + p) / (fs2 - p)).collect();
    let numerator = fs2.powi(analog_zeros);
    let denominator = poles
        .iter()
        .fold(Complex::new(1.0, 0.0), |a, p| a * (fs2 - p));
    gain *= (Complex::new(numerator, 0.0) / denominator).re;
    let mut upper: Vec<Complex<f64>> = digital.into_iter().filter(|p| p.im >= 0.0).collect();
    upper.sort_by(|a, b| {
        (1.0 - a.norm())
            .abs()
            .total_cmp(&(1.0 - b.norm()).abs())
            .reverse()
    });
    upper
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let k = if i == 0 { gain } else { 1.0 };
            [k, -2.0 * zero * k, k, 1.0, -2.0 * p.re, p.norm_sqr()]
        })
        .collect()
}

#[must_use]
pub fn sosfilt(sos: &[Sos], x: &[f64]) -> Vec<f64> {
    let mut y = x.to_vec();
    for [b0, b1, b2, _, a1, a2] in sos {
        let (mut z0, mut z1) = (0.0, 0.0);
        for v in &mut y {
            let input = *v;
            let output = b0 * input + z0;
            z0 = b1 * input - a1 * output + z1;
            z1 = b2 * input - a2 * output;
            *v = output;
        }
    }
    y
}

pub fn firwin2(taps: usize, freq: &[f64], gain: &[f64]) -> Result<Vec<f64>> {
    let nyquist = RATE / 2.0;
    let normalized: Vec<f64> = freq.iter().map(|f| f / nyquist).collect();
    let n = 1 + taps.next_power_of_two();
    let delay = (taps.to_f64().context("taps")? - 1.0) / 2.0;
    let last = (n - 1).to_f64().context("taps")?;
    let spectrum: Vec<Complex<f64>> = (0u32..)
        .take(n)
        .map(|i| {
            let x = f64::from(i) / last;
            Complex::from_polar(interp(x, &normalized, gain), -delay * PI * x)
        })
        .collect();
    let full = irfft(&spectrum, 2 * (n - 1))?;
    Ok(full
        .iter()
        .zip(hamming(taps)?)
        .take(taps)
        .map(|(h, w)| h * w)
        .collect())
}

pub fn minimum_phase(h: &[f64]) -> Result<Vec<f64>> {
    let n = (2.0 * (h.len() - 1).to_f64().context("filter length")? / 0.01)
        .log2()
        .ceil()
        .exp2()
        .to_usize()
        .context("filter length")?;
    let mut spectrum: Vec<Complex<f64>> = (0..n)
        .map(|i| Complex::new(h.get(i).copied().unwrap_or(0.0), 0.0))
        .collect();
    fft(&mut spectrum, false)?;
    let magnitude: Vec<f64> = spectrum.iter().map(|c| c.norm()).collect();
    let floor = 1e-7
        * magnitude
            .iter()
            .copied()
            .filter(|v| *v > 0.0)
            .fold(f64::INFINITY, f64::min);
    let mut cepstrum: Vec<Complex<f64>> = magnitude
        .iter()
        .map(|v| Complex::new((v + floor).ln(), 0.0))
        .collect();
    fft(&mut cepstrum, true)?;
    let stop = n / 2;
    for (i, c) in cepstrum.iter_mut().enumerate() {
        let w = if i == 0 || (n % 2 == 1 && i == stop) {
            1.0
        } else if i < stop {
            2.0
        } else {
            0.0
        };
        *c = Complex::new(c.re * w, 0.0);
    }
    fft(&mut cepstrum, false)?;
    for c in &mut cepstrum {
        *c = c.exp();
    }
    fft(&mut cepstrum, true)?;
    Ok(cepstrum.iter().take(h.len()).map(|c| c.re).collect())
}

pub struct Stft {
    pub frames: Vec<Vec<Complex<f64>>>,
    window: Vec<f64>,
    step: usize,
}

pub fn stft(x: &[f64], n: usize) -> Result<Stft> {
    let step = n - n * 3 / 4;
    let window = hann_periodic(n)?;
    let total: f64 = window.iter().sum();
    let mut padded = vec![0.0; n / 2];
    padded.extend_from_slice(x);
    padded.extend(std::iter::repeat_n(0.0, n / 2));
    let extra = (step - (padded.len() - n) % step) % step;
    padded.extend(std::iter::repeat_n(0.0, extra));
    let frames = (0..=(padded.len() - n) / step)
        .map(|k| {
            let segment: Vec<f64> = padded[k * step..k * step + n]
                .iter()
                .zip(&window)
                .map(|(v, w)| v * w)
                .collect();
            rfft(&segment, n).iter().map(|c| c / total).collect()
        })
        .collect();
    Ok(Stft {
        frames,
        window,
        step,
    })
}

impl Stft {
    pub fn inverse(&self) -> Result<Vec<f64>> {
        let n = self.window.len();
        let total: f64 = self.window.iter().sum();
        let length = n + (self.frames.len() - 1) * self.step;
        let mut out = vec![0.0; length];
        let mut norm = vec![0.0; length];
        for (k, frame) in self.frames.iter().enumerate() {
            let scaled: Vec<Complex<f64>> = frame.iter().map(|c| c * total).collect();
            let segment = irfft(&scaled, n)?;
            for (i, (v, w)) in segment.iter().zip(&self.window).enumerate() {
                out[k * self.step + i] += v * w;
                norm[k * self.step + i] += w * w;
            }
        }
        for (v, w) in out.iter_mut().zip(&norm) {
            if *w > 1e-10 {
                *v /= w;
            }
        }
        Ok(out[n / 2..length - n / 2].to_vec())
    }
}

fn bessel_i0(x: f64) -> f64 {
    let mut term = 1.0;
    let mut total = 1.0;
    let quarter = x * x / 4.0;
    for k in 1..200 {
        term *= quarter / f64::from(k * k);
        total += term;
        if term < total * 1e-17 {
            break;
        }
    }
    total
}

fn kaiser_lowpass(taps: usize, cutoff: f64) -> Result<Vec<f64>> {
    let last = (taps - 1).to_f64().context("taps")?;
    let middle = last / 2.0;
    let h: Vec<f64> = (0u32..)
        .take(taps)
        .map(f64::from)
        .map(|i| {
            let m = i - middle;
            let x = cutoff * m;
            let sinc = if x == 0.0 {
                1.0
            } else {
                (PI * x).sin() / (PI * x)
            };
            let r = (2.0 * i / last) - 1.0;
            cutoff * sinc * bessel_i0(5.0 * (1.0 - r * r).max(0.0).sqrt()) / bessel_i0(5.0)
        })
        .collect();
    let total: f64 = h.iter().sum();
    Ok(h.iter().map(|v| v / total).collect())
}

pub fn resample_poly(x: &[f64], up: usize, down: usize) -> Result<Vec<f64>> {
    if up == down {
        return Ok(x.to_vec());
    }
    let rate = up.max(down);
    let half = 10 * rate;
    let gain = up.to_f64().context("resampling factor")?;
    let mut h: Vec<f64> = kaiser_lowpass(
        2 * half + 1,
        1.0 / rate.to_f64().context("resampling factor")?,
    )?
    .iter()
    .map(|v| v * gain)
    .collect();
    let pre = down - half % down;
    let out_len = (x.len() * up).div_ceil(down);
    let remove = (half + pre) / down;
    let mut post = 0;
    let output_len = |taps: usize| ((x.len() - 1) * up + taps - 1) / down + 1;
    while output_len(h.len() + pre + post) < out_len + remove {
        post += 1;
    }
    let mut padded = vec![0.0; pre];
    padded.append(&mut h);
    padded.extend(std::iter::repeat_n(0.0, post));
    Ok((remove..remove + out_len)
        .map(|j| {
            let position = j * down;
            let first = position % up;
            (first..padded.len().min(position + 1))
                .step_by(up)
                .map(|k| {
                    let i = (position - k) / up;
                    x.get(i).map_or(0.0, |v| padded[k] * v)
                })
                .sum()
        })
        .collect())
}

#[must_use]
pub fn channel(x: &[Frame], c: usize) -> Vec<f64> {
    x.iter().map(|f| f[c]).collect()
}

#[must_use]
pub fn join(left: &[f64], right: &[f64]) -> Vec<Frame> {
    left.iter().zip(right).map(|(l, r)| [*l, *r]).collect()
}

#[must_use]
pub fn slice<T>(x: &[T], start: f64, end: f64) -> &[T] {
    let clamp = |t: f64| {
        (t * RATE)
            .max(0.0)
            .to_usize()
            .unwrap_or(usize::MAX)
            .min(x.len())
    };
    let (a, b) = (clamp(start), clamp(end));
    &x[a..b.max(a)]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn butterworth_matches_scipy() {
        let low = butter(2, 0.1, false);
        let expected = [
            0.020_083_365_564_211_2,
            0.040_166_731_128_422_4,
            0.020_083_365_564_211_2,
            1.0,
            -1.561_018_075_800_718,
            0.641_351_538_057_563_2,
        ];
        for (a, b) in low[0].iter().zip(expected) {
            assert!((a - b).abs() < 1e-12, "{low:?}");
        }
        let high = butter(2, 0.1, true);
        let expected = [
            0.800_592_403_464_570_4,
            -1.601_184_806_929_141,
            0.800_592_403_464_570_4,
            1.0,
            -1.561_018_075_800_718,
            0.641_351_538_057_563_2,
        ];
        for (a, b) in high[0].iter().zip(expected) {
            assert!((a - b).abs() < 1e-12, "{high:?}");
        }
    }

    #[test]
    fn fourth_order_lowpass_has_unit_dc_gain() {
        let sos = butter(4, 150.0 / (RATE / 2.0), false);
        let y = sosfilt(&sos, &vec![1.0; 200_000]);
        assert!((y[y.len() - 1] - 1.0).abs() < 1e-9);
    }

    #[test]
    fn stft_round_trips() {
        let x: Vec<f64> = (0..5000u32).map(|i| (f64::from(i) * 0.01).sin()).collect();
        let back = stft(&x, 512).unwrap().inverse().unwrap();
        for (a, b) in x.iter().zip(&back) {
            assert!((a - b).abs() < 1e-9);
        }
    }

    #[test]
    fn resample_keeps_a_low_tone() {
        let x: Vec<f64> = (0..4000u32)
            .map(|i| (2.0 * PI * 440.0 * f64::from(i) / RATE).sin())
            .collect();
        let up = resample_poly(&x, 4, 1).unwrap();
        assert_eq!(up.len(), 16_000);
        for i in (2000..14_000u32).step_by(97) {
            let expected = (2.0 * PI * 440.0 * f64::from(i) / (4.0 * RATE)).sin();
            assert!((up[i as usize] - expected).abs() < 1e-3, "{i}");
        }
        let down = resample_poly(&up, 1, 4).unwrap();
        for i in 500..3500 {
            assert!((down[i] - x[i]).abs() < 1e-3);
        }
    }

    #[test]
    fn minimum_phase_keeps_the_magnitude_of_a_flat_filter() {
        let h = firwin2(255, &[0.0, RATE / 2.0], &[1.0, 1.0]).unwrap();
        let m = minimum_phase(&h).unwrap();
        let energy: f64 = m.iter().map(|v| v * v).sum();
        assert!((energy - 1.0).abs() < 1e-3, "{energy}");
        assert!(m[0].abs() > 0.9);
    }

    #[test]
    fn moving_mean_matches_numpy_same() {
        let x = [1.0, 2.0, 3.0, 4.0, 5.0];
        let y = moving_mean_same(&x, 3).unwrap();
        let expected = [1.0, 2.0, 3.0, 4.0, 3.0];
        for (a, b) in y.iter().zip(expected) {
            assert!((a - b).abs() < 1e-12, "{y:?}");
        }
    }
}
