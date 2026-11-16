use std::path::Path;

use anyhow::{Context, Result};
use num_traits::ToPrimitive;
use serde_json::{Value, json};

use crate::place::{compare, place};
use crate::signal::{
    Format, Frame, RATE, apply_gain, box_smooth_edge, db, interp, load, mean_square, mono, rms,
    round, write,
};
use crate::song::Song;

const BINS: usize = 48;

#[must_use]
pub fn rms_db(x: &[f64]) -> f64 {
    db(rms(x) + 1e-12)
}

pub fn bar_gain(song: &Song, y: &[Frame], reference: &[Frame]) -> Result<(Vec<Frame>, Vec<f64>)> {
    let (limit, silent) = (12.0, -70.0);
    let step = (song.bar * RATE).to_usize().context("bar length")?;
    let (ym, rm) = (mono(y), mono(reference));
    let gains: Vec<f64> = (0..y.len().div_ceil(step))
        .map(|i| {
            let span =
                |m: &[f64]| m[(i * step).min(m.len())..((i + 1) * step).min(m.len())].to_vec();
            let a = rms_db(&span(&ym));
            let r = rms_db(&span(&rm));
            if r < silent {
                -120.0
            } else if a > silent {
                (r - a).clamp(-limit, limit)
            } else {
                0.0
            }
        })
        .collect();
    let mut linear: Vec<f64> = gains
        .iter()
        .flat_map(|g| std::iter::repeat_n(10f64.powf(g / 20.0), step))
        .collect();
    linear.truncate(y.len());
    let smooth = box_smooth_edge(&linear, (0.005 * RATE).to_usize().context("smoothing")?)?;
    Ok((
        apply_gain(y, &smooth),
        gains.iter().map(|g| round(*g, 1)).collect(),
    ))
}

#[must_use]
pub fn profile(m: &[f64], starts: &[usize], hop: usize, bins: usize) -> Vec<f64> {
    let mut acc = vec![0.0; bins];
    let mut k = 0;
    for &s in starts {
        if s + hop * bins <= m.len() {
            for (j, a) in acc.iter_mut().enumerate() {
                *a += mean_square(&m[s + j * hop..s + (j + 1) * hop]);
            }
            k += 1;
        }
    }
    let p: Vec<f64> = acc
        .iter()
        .map(|a| 10.0 * (a / f64::from(k.max(1)) + 1e-20).log10())
        .collect();
    let top = p.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    p.iter().map(|v| v - top).collect()
}

pub fn beat_gain(song: &Song, y: &[Frame], reference: &[Frame]) -> Result<(Vec<Frame>, Vec<f64>)> {
    let (lo, hi) = (-24.0, 6.0);
    let rm = mono(reference);
    let step = song.beat * RATE;
    let bins = BINS.to_f64().context("bins")?;
    let beats = (rm.len().to_f64().context("length")? / step)
        .to_usize()
        .context("beat count")?;
    let sample = |beat: u32| (f64::from(beat) * step).to_usize().context("beat start");
    let level: Vec<f64> = (0u32..)
        .take(beats)
        .map(|i| Ok(rms_db(&rm[sample(i)?..sample(i + 1)?])))
        .collect::<Result<_>>()?;
    let top = level.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let hop = step.to_usize().context("beat length")? / BINS;
    let starts: Vec<usize> = level
        .iter()
        .zip(0u32..)
        .filter(|(v, _)| **v > top - 30.0)
        .map(|(_, i)| {
            (f64::from(i) * step)
                .round_ties_even()
                .to_usize()
                .context("beat start")
        })
        .collect::<Result<_>>()?;
    let g: Vec<f64> = profile(&rm, &starts, hop, BINS)
        .iter()
        .zip(profile(&mono(y), &starts, hop, BINS))
        .map(|(r, c)| (r - c).clamp(lo, hi))
        .collect();
    let g: Vec<f64> = (0..BINS)
        .map(|i| 0.25 * g[(i + BINS - 1) % BINS] + 0.5 * g[i] + 0.25 * g[(i + 1) % BINS])
        .collect();
    let mut xp = vec![-0.5];
    xp.extend((0u32..).take(BINS).map(|i| f64::from(i) + 0.5));
    xp.push(bins + 0.5);
    let mut fp = vec![g[BINS - 1]];
    fp.extend_from_slice(&g);
    fp.push(g[0]);
    let curve: Vec<f64> = (0u32..)
        .take(y.len())
        .map(|i| {
            let phase = (f64::from(i) / step) % 1.0 * bins;
            10f64.powf(interp(phase, &xp, &fp) / 20.0)
        })
        .collect();
    Ok((
        apply_gain(y, &curve),
        g.iter().map(|v| round(*v, 1)).collect(),
    ))
}

pub fn run(song: &Song, stem: &str, raw: &Path, out: &Path, bars_only: bool) -> Result<Value> {
    let n = song.frames()?;
    let raw = load(raw, Some(n))?;
    let reference = song.stem(stem, Some(n))?;
    let (mut y, bars) = bar_gain(song, &raw, &reference)?;
    let mut phase = Vec::new();
    if !bars_only {
        (y, phase) = beat_gain(song, &y, &reference)?;
    }
    let (y, moves) = place(&y, &reference, 2)?;
    write(out, &y, Format::Float)?;
    Ok(json!({
        "stem": stem,
        "bar_gain_db": bars,
        "beat_phase_gain_db": phase,
        "moves": moves,
        "before": compare(&raw, &reference)?,
        "after": compare(&y, &reference)?,
    }))
}
