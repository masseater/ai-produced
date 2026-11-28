use anyhow::{Context, Result};
use num_traits::ToPrimitive;

use crate::shared::shaping::profile;
use crate::shared::signal::{RATE, mean};

pub fn note_profile(m: &[f64], onsets: &[f64], hop: usize, bins: usize) -> Result<Vec<f64>> {
    let starts: Vec<usize> = onsets
        .iter()
        .filter(|t| **t >= 0.0)
        .map(|t| (t * RATE).to_usize().context("onset"))
        .collect::<Result<_>>()?;
    Ok(profile(m, &starts, hop, bins))
}

pub fn note_env(m: &[f64], onsets: &[f64]) -> Result<Vec<f64>> {
    Ok(note_profile(m, onsets, 220, 80)?
        .iter()
        .map(|v| v.max(-50.0))
        .collect())
}

pub fn mean_abs_diff(a: &[f64], b: &[f64]) -> Result<f64> {
    mean(a.iter().zip(b).map(|(x, y)| (x - y).abs()))
}
