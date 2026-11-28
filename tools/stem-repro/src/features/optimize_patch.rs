use std::cell::RefCell;
use std::path::Path;

use anyhow::{Context, Result};
use serde_json::{Map, Value, json};

use crate::shared::note_profile::{mean_abs_diff, note_env};
use crate::shared::notes::{events, shifted};
use crate::shared::optim::differential_evolution;
use crate::shared::signal::{mono, round};
use crate::shared::song::Song;
use crate::shared::spectral::{chroma, dist3};
use crate::shared::surge::{Surge, install};

const PARAMS: [&str; 8] = [
    "a_amp_eg_attack",
    "a_amp_eg_decay",
    "a_amp_eg_sustain",
    "a_amp_eg_release",
    "a_filter_1_cutoff",
    "a_filter_1_feg_mod_amount",
    "a_filter_eg_decay",
    "a_filter_eg_sustain",
];
const SHIFT_MS: f64 = 12.0;

pub fn run(song: &Song, part: &str, patch: &Path, max_iter: u64, popsize: usize) -> Result<Value> {
    let [b0, b1] = song
        .part(part)?
        .rescore_bars
        .context("the part has no rescore_bars")?;
    let (t0, t1) = (b0 * song.bar, b1 * song.bar);
    let notes = song.notes_in(&song.part_notes(part)?, b0, b1);
    let reference = mono(song.bars(&song.stem(part, None)?, b0, b1));
    let onsets: Vec<f64> = notes.iter().map(|n| n.start - t0).collect();
    let (env_ref, chroma_ref) = (note_env(&reference, &onsets)?, chroma(&reference)?);
    let synth = RefCell::new(Surge::new(&install()?)?);
    synth.borrow_mut().load(patch, &[])?;
    let mut x0: Vec<f64> = PARAMS
        .iter()
        .map(|p| synth.borrow_mut().value(p))
        .collect::<Result<_>>()?;
    x0.push(0.0);
    let terms = |x: &[f64]| -> Option<Vec<f64>> {
        let mut s = synth.borrow_mut();
        for (p, v) in PARAMS.iter().zip(x) {
            s.set(p, *v).ok()?;
        }
        let sh = x[PARAMS.len()] / 1000.0;
        let m = s
            .render_mono(
                &events(&shifted(&notes, sh), t0, Some(t1)),
                t1 - t0,
                reference.len(),
            )
            .ok()??;
        let (sp, en, ch) = dist3(&m, &reference, &chroma_ref).ok()?;
        let ne = mean_abs_diff(&note_env(&m, &onsets).ok()?, &env_ref).ok()?;
        Some([sp, en, ch, ne].iter().map(|v| round(*v, 3)).collect())
    };
    let before = terms(&x0);
    let mut bounds = vec![(0.0, 1.0); PARAMS.len()];
    bounds.push((-SHIFT_MS, SHIFT_MS));
    let best = differential_evolution(
        |x| terms(x).map_or(99.0, |t| t.iter().sum()),
        &bounds,
        popsize,
        max_iter,
        1,
    )?;
    let params: Map<String, Value> = PARAMS
        .iter()
        .zip(&best.x)
        .map(|(n, v)| ((*n).to_string(), json!(v)))
        .collect();
    Ok(json!({
        "part": part,
        "patch": patch,
        "spec_env_chroma_noteenv_before": before,
        "spec_env_chroma_noteenv": terms(&best.x),
        "loss": round(best.f, 3),
        "shift_ms": round(best.x[PARAMS.len()], 2),
        "params": params,
    }))
}
