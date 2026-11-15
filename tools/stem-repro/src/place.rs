use std::path::Path;

use anyhow::{Context, Result};
use num_traits::ToPrimitive;
use serde::Serialize;
use serde_json::{Value, json};

use crate::eq::{curve, matched};
use crate::features::dist;
use crate::signal::{
    Format, Frame, RATE, Stats, butter, dual, from_mid_side, load, lr_db, lufs, mid_side, mono,
    rms, round, scale, side_mid_db, sosfilt, stats, write,
};
use crate::song::Song;

#[derive(Serialize, Clone)]
pub struct Comparison {
    pub spec_db: f64,
    pub env_db: f64,
    pub cand: Stats,
    #[serde(rename = "ref")]
    pub reference: Stats,
    pub eq_needed: Vec<i64>,
}

pub fn compare(cand: &[Frame], reference: &[Frame]) -> Result<Comparison> {
    let (s, e) = dist(&mono(cand), &mono(reference))?;
    Ok(Comparison {
        spec_db: round(s, 2),
        env_db: round(e, 2),
        cand: stats(cand),
        reference: stats(reference),
        eq_needed: rounded(&curve(cand, reference)?)?,
    })
}

pub fn rounded(values: &[f64]) -> Result<Vec<i64>> {
    values
        .iter()
        .map(|v| v.round_ties_even().to_i64().context("EQ gain"))
        .collect()
}

fn widen(y: &[Frame], target: f64) -> Result<Vec<Frame>> {
    let (m, s) = mid_side(y);
    let current = side_mid_db(y);
    let side: Vec<f64> = if current < target - 3.0 {
        let hp = sosfilt(&butter(2, 200.0 / (RATE / 2.0), true), &m);
        let delay = (0.011 * RATE).to_usize().context("delay")?;
        let mut delayed = vec![0.0; delay.min(hp.len())];
        delayed.extend_from_slice(&hp[..hp.len().saturating_sub(delay)]);
        let gain = 10f64.powf(target / 20.0) * rms(&m) / (rms(&delayed) + 1e-12);
        delayed.iter().map(|v| v * gain).collect()
    } else {
        let gain = 10f64.powf((target - current) / 20.0);
        s.iter().map(|v| v * gain).collect()
    };
    Ok(from_mid_side(&m, &side))
}

fn balance(y: &[Frame], target: f64) -> Vec<Frame> {
    let d = (target - lr_db(y)) / 2.0;
    let (gl, gr) = (10f64.powf(d / 20.0), 10f64.powf(-d / 20.0));
    y.iter().map(|[l, r]| [l * gl, r * gr]).collect()
}

#[derive(Serialize, Clone)]
pub struct Moves {
    pub eq_applied_db: Vec<i64>,
    pub gain_db: f64,
}

pub fn place(raw: &[Frame], reference: &[Frame], passes: usize) -> Result<(Vec<Frame>, Moves)> {
    let (y, eq) = matched(raw, reference, passes)?;
    let target = side_mid_db(reference);
    let y = if target > -60.0 {
        widen(&y, target)?
    } else {
        dual(&mono(&y))
    };
    let y = balance(&y, lr_db(reference));
    let gain = match (lufs(reference), lufs(&y)) {
        (Some(a), Some(b)) => a - b,
        _ => 0.0,
    };
    Ok((
        scale(&y, 10f64.powf(gain / 20.0)),
        Moves {
            eq_applied_db: rounded(&eq)?,
            gain_db: round(gain, 1),
        },
    ))
}

pub fn run(song: &Song, stem: &str, raw: &Path, out: &Path) -> Result<Value> {
    let n = song.frames()?;
    let raw = load(raw, Some(n))?;
    let reference = song.stem(stem, Some(n))?;
    let before = compare(&raw, &reference)?;
    let (y, moves) = place(&raw, &reference, 2)?;
    write(out, &y, Format::Float)?;
    Ok(json!({
        "stem": stem,
        "before": before,
        "moves": moves,
        "after": compare(&y, &reference)?,
    }))
}
