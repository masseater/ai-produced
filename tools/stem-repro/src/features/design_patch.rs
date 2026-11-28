use std::cell::RefCell;
use std::path::Path;

use anyhow::{Context, Result};
use num_traits::ToPrimitive;
use serde_json::{Map, Value, json};

use crate::shared::note_profile::{mean_abs_diff, note_env};
use crate::shared::notes::{Note, events, read, shifted};
use crate::shared::optim::cma_es;
use crate::shared::signal::{Format, RATE, hanning, mean, mono, rfft, round, write};
use crate::shared::song::Song;
use crate::shared::spectral::dist;
use crate::shared::surge::{Surge, install};

pub const FILTERS: [(&str, f64); 9] = [
    ("off", 0.0),
    ("lp12", 0.025),
    ("lp24", 0.05),
    ("ladder", 0.085),
    ("vintage", 0.295),
    ("obxd12", 0.325),
    ("obxd24", 0.355),
    ("k35", 0.385),
    ("diode", 0.445),
];
const PARAMS: [&str; 15] = [
    "a_osc_1_shape",
    "a_osc_1_width_1",
    "a_osc_1_sub_mix",
    "a_filter_1_cutoff",
    "a_filter_1_resonance",
    "a_filter_1_feg_mod_amount",
    "a_filter_eg_attack",
    "a_filter_eg_decay",
    "a_filter_eg_sustain",
    "a_filter_eg_release",
    "a_amp_eg_attack",
    "a_amp_eg_decay",
    "a_amp_eg_sustain",
    "a_amp_eg_release",
    "a_waveshaper_drive",
];
const NOISE: [&str; 3] = ["a_noise_volume", "a_noise_color", "a_osc_1_unison_detune"];
const SHIFT_MS: f64 = 12.0;

fn hz(pitch: u8) -> f64 {
    440.0 * ((f64::from(pitch) - 69.0) / 12.0).exp2()
}

fn harmonics(m: &[f64], onsets: &[f64], f0s: &[f64]) -> Result<Vec<Vec<f64>>> {
    let (partials, frames, nfft) = (12, 15, 4096u32);
    let samples = |seconds: f64| (seconds * RATE).to_usize().context("window");
    let (width, step) = (samples(0.04)?, samples(0.01)?);
    let window = hanning(width)?;
    let bin = RATE / f64::from(nfft);
    let mut acc = vec![vec![0.0; partials]; frames];
    let mut count = 0;
    for (onset, f0) in onsets.iter().zip(f0s) {
        let start = (onset * RATE).trunc();
        if start < 0.0 {
            continue;
        }
        let first = start.to_usize().context("onset")?;
        if first + (frames - 1) * step + width > m.len() {
            continue;
        }
        for (frame, row) in acc.iter_mut().enumerate() {
            let at = first + frame * step;
            let segment: Vec<f64> = m[at..at + width]
                .iter()
                .zip(&window)
                .map(|(v, w)| v * w)
                .collect();
            let power: Vec<f64> = rfft(&segment, nfft as usize)
                .iter()
                .map(rustfft::num_complex::Complex::norm_sqr)
                .collect();
            for (value, harmonic) in row.iter_mut().zip(1u32..) {
                let position = f0 * f64::from(harmonic) / bin;
                let lo = position.floor().to_usize().context("harmonic")?;
                if lo + 1 < power.len() {
                    let fraction = position - position.floor();
                    *value += power[lo] + fraction * (power[lo + 1] - power[lo]);
                }
            }
        }
        count += 1;
    }
    let levels: Vec<Vec<f64>> = acc
        .iter()
        .map(|r| {
            r.iter()
                .map(|v| 10.0 * (v / f64::from(count.max(1)) + 1e-20).log10())
                .collect()
        })
        .collect();
    let top = levels
        .iter()
        .flatten()
        .copied()
        .fold(f64::NEG_INFINITY, f64::max);
    Ok(levels
        .iter()
        .map(|r| r.iter().map(|v| (v - top).max(-60.0)).collect())
        .collect())
}

fn harmonic_term(h: &[Vec<f64>], reference: &[Vec<f64>]) -> Result<f64> {
    let diff: Vec<Vec<f64>> = h
        .iter()
        .zip(reference)
        .map(|(a, b)| a.iter().zip(b).map(|(x, y)| x - y).collect())
        .collect();
    let columns = diff.first().map_or(0, Vec::len);
    let means: Vec<f64> = (0..columns)
        .map(|c| mean(diff.iter().map(|r| r[c])))
        .collect::<Result<_>>()?;
    mean(
        diff.iter()
            .flat_map(|r| r.iter().zip(&means).map(|(v, m)| (v - m).abs())),
    )
}

pub struct Design<'a> {
    pub part: &'a str,
    pub template: &'a str,
    pub filter: &'a str,
    pub bars: [f64; 2],
    pub iterations: u64,
    pub noise: bool,
    pub midi: Option<&'a Path>,
}

pub fn run(song: &Song, design: &Design) -> Result<Value> {
    let [b0, b1] = design.bars;
    let (t0, t1) = (b0 * song.bar, b1 * song.bar);
    let all: Vec<Note> = match design.midi {
        Some(m) => read(m, None)?,
        None => song.part_notes(design.part)?,
    };
    let notes = song.notes_in(&all, b0, b1);
    let reference = mono(song.bars(&song.stem(design.part, None)?, b0, b1));
    let onsets: Vec<f64> = notes.iter().map(|n| n.start - t0).collect();
    let f0s: Vec<f64> = notes.iter().map(|n| hz(n.pitch)).collect();
    let (h_ref, env_ref) = (
        harmonics(&reference, &onsets, &f0s)?,
        note_env(&reference, &onsets)?,
    );
    let filter_value = FILTERS
        .iter()
        .find(|(n, _)| *n == design.filter)
        .map(|(_, v)| *v)
        .with_context(|| format!("unknown filter {}", design.filter))?;
    let mut fixed = vec![
        ("a_filter_1_type".to_string(), filter_value),
        (
            "a_waveshaper_type".to_string(),
            if design.filter == "off" { 0.0 } else { 0.02 },
        ),
    ];
    if design.noise {
        fixed.push(("a_noise_mute".to_string(), 0.0));
    }
    let names: Vec<&str> = PARAMS
        .iter()
        .chain(if design.noise { NOISE.as_slice() } else { &[] })
        .copied()
        .collect();
    let install = install()?;
    let patch = install
        .patches
        .join("patches_factory")
        .join("Templates")
        .join(format!("{}.fxp", design.template));
    let synth = RefCell::new(Surge::new(&install)?);
    synth.borrow_mut().load(&patch, &fixed)?;
    let mut x0: Vec<f64> = names
        .iter()
        .map(|p| synth.borrow_mut().value(p))
        .collect::<Result<_>>()?;
    x0.push(0.5);
    let x0: Vec<f64> = x0.iter().map(|v| v.clamp(0.05, 0.95)).collect();
    let shift = |x: &[f64]| (x[names.len()] - 0.5) * 2.0 * SHIFT_MS;
    let terms = |x: &[f64]| -> Option<Vec<f64>> {
        let mut s = synth.borrow_mut();
        for (p, v) in names.iter().zip(x) {
            s.set(p, v.clamp(0.0, 1.0)).ok()?;
        }
        let m = s
            .render_mono(
                &events(&shifted(&notes, shift(x) / 1000.0), t0, Some(t1)),
                t1 - t0,
                reference.len(),
            )
            .ok()??;
        let (sp, en) = dist(&m, &reference).ok()?;
        Some(vec![
            sp,
            en,
            harmonic_term(&harmonics(&m, &onsets, &f0s).ok()?, &h_ref).ok()?,
            mean_abs_diff(&note_env(&m, &onsets).ok()?, &env_ref).ok()?,
        ])
    };
    let loss = |x: &[f64]| terms(x).map_or(99.0, |t| 0.5 * t[0] + t[1..].iter().sum::<f64>());
    let best = cma_es(loss, x0, 0.25, 12, design.iterations, 1)?;
    let mut params: Map<String, Value> = fixed.iter().map(|(n, v)| (n.clone(), json!(v))).collect();
    for (n, v) in names.iter().zip(&best.x) {
        params.insert((*n).to_string(), json!(v.clamp(0.0, 1.0)));
    }
    Ok(json!({
        "part": design.part,
        "patch": patch,
        "filter": design.filter,
        "loss": round(best.f, 3),
        "spec_env_harmonics_noteenv": terms(&best.x).map(|t| t.iter().map(|v| round(*v, 3)).collect::<Vec<_>>()),
        "shift_ms": round(shift(&best.x), 2),
        "params": params,
    }))
}

pub fn run_render(
    song: &Song,
    part: &str,
    params: &Path,
    midi: Option<&Path>,
    out: &Path,
) -> Result<Value> {
    let pick: Value = serde_json::from_str(&std::fs::read_to_string(params)?)?;
    let notes = match midi {
        Some(m) => read(m, None)?,
        None => song.part_notes(part)?,
    };
    let sh = pick["shift_ms"].as_f64().unwrap_or(0.0) / 1000.0;
    let values: Vec<(String, f64)> = pick["params"]
        .as_object()
        .context("params file has no params object")?
        .iter()
        .filter_map(|(k, v)| v.as_f64().map(|v| (k.clone(), v)))
        .collect();
    let patch = pick["patch"].as_str().context("params file has no patch")?;
    let mut synth = Surge::new(&install()?)?;
    synth.load(Path::new(patch), &values)?;
    let n = song.frames()?;
    let duration = n.to_f64().context("song length")? / RATE;
    let mut y = synth.render(&events(&shifted(&notes, sh), 0.0, None), duration)?;
    y.resize(n, [0.0; 2]);
    write(out, &y, Format::Float)?;
    Ok(json!({"part": part, "notes": notes.len(), "out": out}))
}
