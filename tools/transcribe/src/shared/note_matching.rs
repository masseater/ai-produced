use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use num_traits::ToPrimitive;
use serde_json::{Value, json};

use crate::shared::midi::{ScoredNote, read};

const ONSET_TOLERANCE: f64 = 0.05;
const OFFSET_MIN_TOLERANCE: f64 = 0.05;
const STEP: f64 = 0.005;

pub struct Selection<'a> {
    pub ref_instrument: Option<usize>,
    pub est_programs: Option<&'a [u8]>,
    pub max_shift: f64,
}

fn load(path: &Path, track: Option<usize>, programs: Option<&[u8]>) -> Result<Vec<ScoredNote>> {
    Ok(read(&fs::read(path)?)?
        .into_iter()
        .enumerate()
        .filter(|(i, inst)| {
            !inst.is_drum
                && track.is_none_or(|t| t == *i)
                && programs.is_none_or(|p| p.contains(&inst.program))
        })
        .flat_map(|(_, inst)| inst.notes)
        .map(|n| ScoredNote {
            end: n.end.max(n.start + 0.01),
            ..n
        })
        .collect())
}

fn round7(value: f64) -> f64 {
    (value * 1e7).round_ties_even() / 1e7
}

fn augment(
    est: usize,
    edges: &[Vec<usize>],
    owner: &mut [Option<usize>],
    seen: &mut [bool],
) -> bool {
    for &r in &edges[est] {
        if seen[r] {
            continue;
        }
        seen[r] = true;
        if owner[r].is_none_or(|other| augment(other, edges, owner, seen)) {
            owner[r] = Some(est);
            return true;
        }
    }
    false
}

fn score(
    reference: &[ScoredNote],
    estimate: &[ScoredNote],
    shift: f64,
    offset_ratio: Option<f64>,
) -> Result<(f64, f64, f64)> {
    if reference.is_empty() || estimate.is_empty() {
        return Ok((0.0, 0.0, 0.0));
    }
    let edges: Vec<Vec<usize>> = estimate
        .iter()
        .map(|e| {
            reference
                .iter()
                .enumerate()
                .filter(|(_, r)| {
                    let onset = round7((r.start - (e.start + shift)).abs()) <= ONSET_TOLERANCE;
                    let pitch =
                        round7((1200.0 * ((f64::from(r.pitch) - f64::from(e.pitch)) / 12.0)).abs())
                            <= 50.0;
                    let offset = offset_ratio.is_none_or(|ratio| {
                        let tolerance = (ratio * (r.end - r.start)).max(OFFSET_MIN_TOLERANCE);
                        round7((r.end - (e.end + shift)).abs()) <= tolerance
                    });
                    onset && pitch && offset
                })
                .map(|(i, _)| i)
                .collect()
        })
        .collect();
    let mut owner = vec![None; reference.len()];
    let matched = (0..estimate.len())
        .filter(|&e| augment(e, &edges, &mut owner, &mut vec![false; reference.len()]))
        .count()
        .to_f64()
        .context("match count")?;
    let precision = matched / estimate.len().to_f64().context("estimate count")?;
    let recall = matched / reference.len().to_f64().context("reference count")?;
    let f1 = if precision + recall > 0.0 {
        2.0 * precision * recall / (precision + recall)
    } else {
        0.0
    };
    Ok((precision, recall, f1))
}

fn round3(value: f64) -> f64 {
    (value * 1e3).round_ties_even() / 1e3
}

pub fn score_files(reference: &Path, estimate: &Path, selection: &Selection) -> Result<Value> {
    let reference = load(reference, selection.ref_instrument, None)?;
    let estimate = load(estimate, None, selection.est_programs)?;
    let steps = ((2.0 * selection.max_shift + 1e-9) / STEP)
        .floor()
        .to_u32()
        .context("max shift")?;
    let mut shift = (0.0, f64::NEG_INFINITY);
    for k in 0..=steps {
        let s = -selection.max_shift + f64::from(k) * STEP;
        let f = score(&reference, &estimate, s, None)?.2;
        if f > shift.1 {
            shift = (s, f);
        }
    }
    let shift = shift.0;
    let (precision, recall, f1) = score(&reference, &estimate, shift, None)?;
    let (_, _, f1_offset) = score(&reference, &estimate, shift, Some(0.2))?;
    Ok(json!({
        "reference_notes": reference.len(),
        "estimated_notes": estimate.len(),
        "shift_ms": (shift * 1000.0).round_ties_even().to_i64().context("shift")?,
        "precision": round3(precision),
        "recall": round3(recall),
        "f1": round3(f1),
        "f1_with_offset": round3(f1_offset),
    }))
}
