use std::path::Path;

use anyhow::{Context, Result};
use num_traits::ToPrimitive;
use serde_json::{Map, Value, json};

use crate::shared::note_profile::note_profile;
use crate::shared::notes::{Note, read};
use crate::shared::placement::{Comparison, compare, place};
use crate::shared::shaping::{bar_gain, beat_gain};
use crate::shared::signal::{
    Format, Frame, RATE, apply_gain, box_smooth_edge, interp, load, mean, mean_square, mono,
    percentile, round, smooth3_edge, write,
};
use crate::shared::song::Song;

const HOP: f64 = 0.005;

fn frame_db(m: &[f64], hop: usize) -> Vec<f64> {
    m.chunks_exact(hop)
        .map(|c| 20.0 * (mean_square(c).sqrt() + 1e-12).log10())
        .collect()
}

fn sounding(notes: &[Note], reference: &[Frame]) -> Result<Vec<Note>> {
    let e = frame_db(&mono(reference), 441);
    let top = percentile(&e, 99.5)?;
    let mut kept = Vec::new();
    for note in notes {
        let i = (note.start * 100.0)
            .max(0.0)
            .to_usize()
            .context("note start")?;
        let level = e[i.min(e.len())..(i + 5).min(e.len())]
            .iter()
            .copied()
            .fold(f64::NEG_INFINITY, f64::max);
        if level - top >= -30.0 {
            kept.push(*note);
        }
    }
    Ok(kept)
}

fn onsets(notes: &[Note], shift: f64) -> Vec<f64> {
    let mut times: Vec<f64> = notes
        .iter()
        .map(|n| ((n.start + shift) * 1000.0).round_ties_even() / 1000.0)
        .collect();
    times.sort_by(f64::total_cmp);
    times.dedup();
    let mut out: Vec<f64> = Vec::new();
    for t in times {
        if out.last().is_none_or(|last| t - last > 0.01) {
            out.push(t);
        }
    }
    out
}

fn gate(notes: &[Note], shift: f64, release_ms: f64, n: usize) -> Result<Vec<f64>> {
    let mut g = vec![0.0f64; n];
    let length = (release_ms / 1000.0 * RATE * 1.5)
        .to_usize()
        .context("release")?;
    let tail: Vec<f64> = (0u32..)
        .take(length)
        .map(|i| 10f64.powf(-3.0 * (f64::from(i) / RATE) / (release_ms / 1000.0)))
        .collect();
    let attack = (0.001 * RATE).trunc();
    for note in notes {
        let i = (((note.start + shift) * RATE).trunc() - attack)
            .max(0.0)
            .to_usize()
            .context("note start")?;
        let j = ((note.end + shift) * RATE)
            .max(0.0)
            .to_usize()
            .context("note end")?
            .min(n);
        if j <= i {
            continue;
        }
        g[i..j].iter_mut().for_each(|v| *v = 1.0);
        for (k, t) in tail.iter().take(n - j).enumerate() {
            g[j + k] = g[j + k].max(*t);
        }
    }
    Ok(g)
}

fn sample_at(seconds: f64) -> Result<usize> {
    (seconds * RATE)
        .max(0.0)
        .to_usize()
        .with_context(|| format!("bad time {seconds} s"))
}

fn per_note(
    len: usize,
    onsets: &[f64],
    mut curve: impl FnMut(usize, usize, usize) -> Vec<f64>,
) -> Result<Vec<f64>> {
    let mut gain = vec![1.0; len];
    let longest = sample_at(0.5)?;
    for (j, t) in onsets.iter().enumerate() {
        let i = sample_at(*t)?;
        let e = onsets
            .get(j + 1)
            .map_or(Ok(i + longest), |next| sample_at(*next))?
            .min(len);
        if e > i {
            gain[i..e].copy_from_slice(&curve(j, i, e)[..e - i]);
        }
    }
    Ok(gain)
}

fn note_envelope(
    y: &[Frame],
    reference: &[Frame],
    onsets: &[f64],
) -> Result<(Vec<Frame>, Vec<f64>)> {
    let (window, limit) = (0.4, 24.0);
    let hop = sample_at(HOP)?;
    let half = hop.to_f64().context("hop")? / 2.0;
    let bins = (window / HOP).to_usize().context("bins")?;
    let mut total = vec![0.0; bins];
    let mut y = y.to_vec();
    let rm = mono(reference);
    for _ in 0..2 {
        let g: Vec<f64> = note_profile(&rm, onsets, hop, bins)?
            .iter()
            .zip(note_profile(&mono(&y), onsets, hop, bins)?)
            .map(|(r, c)| (r - c).clamp(-limit, limit))
            .collect();
        let g = smooth3_edge(&g);
        for (t, v) in total.iter_mut().zip(&g) {
            *t += v;
        }
        let xp: Vec<f64> = (0u32..)
            .take(bins)
            .map(|i| f64::from(i) * 2.0 * half + half)
            .collect();
        let curve: Vec<f64> = (0u32..)
            .take(hop * bins)
            .map(|i| 10f64.powf(interp(f64::from(i), &xp, &g) / 20.0))
            .collect();
        let gain = per_note(y.len(), onsets, |_, i, e| {
            let n = e - i;
            let mut piece = curve[..n.min(curve.len())].to_vec();
            piece.resize(n, curve[curve.len() - 1]);
            piece
        })?;
        y = apply_gain(&y, &gain);
    }
    Ok((y, total.iter().step_by(4).map(|v| round(*v, 1)).collect()))
}

fn note_gain(y: &[Frame], reference: &[Frame], onsets: &[f64]) -> Result<(Vec<Frame>, Value)> {
    let limit = 12.0;
    let (ym, rm) = (mono(y), mono(reference));
    let mut gains = Vec::new();
    let gain = per_note(y.len(), onsets, |_, i, e| {
        let g = (20.0
            * ((mean_square(&rm[i..e]).sqrt() + 1e-9) / (mean_square(&ym[i..e]).sqrt() + 1e-9))
                .log10())
        .clamp(-limit, limit);
        gains.push(g);
        vec![10f64.powf(g / 20.0); e - i]
    })?;
    let gain = box_smooth_edge(&gain, sample_at(0.003)?)?;
    let average = mean(gains.iter().copied())?;
    let sd = mean(gains.iter().map(|g| (g - average).powi(2)))?.sqrt();
    Ok((
        apply_gain(y, &gain),
        json!({"mean_db": round(average, 2), "sd_db": round(sd, 2)}),
    ))
}

fn silent_mask(reference: &[Frame]) -> Result<Vec<f64>> {
    let m = mono(reference);
    let hop = sample_at(HOP)?;
    let frame = 2 * hop;
    let mut padded = vec![0.0; frame / 2];
    padded.extend_from_slice(&m);
    padded.extend(std::iter::repeat_n(0.0, frame / 2));
    let mut g = vec![0.0; m.len()];
    for f in 0..=(padded.len() - frame) / hop {
        let level = mean_square(&padded[f * hop..f * hop + frame]).sqrt();
        if 20.0 * level.max(1e-5).log10() > -80.0 {
            let (a, b) = ((f * hop).min(m.len()), ((f + 1) * hop).min(m.len()));
            g[a..b].iter_mut().for_each(|v| *v = 1.0);
        }
    }
    box_smooth_edge(&g, sample_at(2.0 / 1000.0)?)
}

fn score(c: &Comparison) -> f64 {
    c.spec_db + c.env_db
}

struct Candidate {
    score: f64,
    how: String,
    y: Vec<Frame>,
    moves: Map<String, Value>,
}

fn candidates(
    song: &Song,
    raw: &[Frame],
    reference: &[Frame],
    notes: &[Note],
    shift: f64,
) -> Result<Vec<Candidate>> {
    let ons = onsets(notes, shift);
    let placed = place(raw, reference, 2)?.0;
    let mut out = Vec::new();
    for pre in [None, Some(10.0), Some(30.0), Some(80.0)] {
        let y0 = match pre {
            None => placed.clone(),
            Some(ms) => {
                place(
                    &apply_gain(raw, &gate(notes, shift, ms, raw.len())?),
                    reference,
                    2,
                )?
                .0
            }
        };
        for envelope in [false, true] {
            let (y1, moves) = if envelope {
                let (y, m) = note_envelope(&y0, reference, &ons)?;
                (y, json!(m))
            } else {
                (y0.clone(), Value::Null)
            };
            let barred = bar_gain(song, &y1, reference)?.0;
            let beat = beat_gain(song, &barred, reference)?.0;
            for (post, z) in [("", y1.clone()), ("bars", barred), ("bars+beat", beat)] {
                let z = if envelope || !post.is_empty() {
                    place(&z, reference, 1)?.0
                } else {
                    z
                };
                let gate_name = pre.map_or_else(String::new, |ms| format!("gate{ms}"));
                let how: Vec<&str> = [
                    "place",
                    &gate_name,
                    if envelope { "noteenv" } else { "" },
                    post,
                ]
                .into_iter()
                .filter(|s| !s.is_empty())
                .collect();
                let mut m = Map::new();
                m.insert("note_env_db".into(), moves.clone());
                out.push(Candidate {
                    score: score(&compare(&z, reference)?),
                    how: how.join("+"),
                    y: z,
                    moves: m,
                });
            }
        }
    }
    out.sort_by(|a, b| a.score.total_cmp(&b.score));
    let top: Vec<(String, Vec<Frame>, Map<String, Value>)> = out
        .iter()
        .take(3)
        .map(|c| (c.how.clone(), c.y.clone(), c.moves.clone()))
        .collect();
    for (how, z, moves) in top {
        let (zn, gains) = note_gain(&z, reference, &ons)?;
        let zn = place(&zn, reference, 1)?.0;
        let mut m = moves;
        m.insert("note_gain".into(), gains);
        out.push(Candidate {
            score: score(&compare(&zn, reference)?),
            how: format!("{how}+notegain"),
            y: zn,
            moves: m,
        });
    }
    let mask = silent_mask(reference)?;
    let mut masked = Vec::with_capacity(out.len());
    for c in &out {
        let zm = apply_gain(&place(&apply_gain(&c.y, &mask), reference, 1)?.0, &mask);
        masked.push(Candidate {
            score: score(&compare(&zm, reference)?),
            how: format!("{}+silence", c.how),
            y: zm,
            moves: c.moves.clone(),
        });
    }
    out.extend(masked);
    Ok(out)
}

pub fn run(
    song: &Song,
    stem: &str,
    raw: &Path,
    out: &Path,
    part: Option<&str>,
    midi: Option<&Path>,
    shift_ms: f64,
) -> Result<Value> {
    let n = song.frames()?;
    let raw = load(raw, Some(n))?;
    let reference = song.stem(stem, Some(n))?;
    let notes = match (midi, part) {
        (Some(m), _) => read(m, None)?,
        (None, Some(p)) => song.part_notes(p)?,
        (None, None) => anyhow::bail!("note-shape needs --midi or --part"),
    };
    let kept = sounding(&notes, &reference)?;
    let mut all = candidates(song, &raw, &reference, &kept, shift_ms / 1000.0)?;
    all.sort_by(|a, b| a.score.total_cmp(&b.score));
    let best = &all[0];
    write(out, &best.y, Format::Float)?;
    let alternatives: Map<String, Value> = all
        .iter()
        .map(|c| (c.how.clone(), json!(round(c.score, 2))))
        .collect();
    Ok(json!({
        "stem": stem,
        "notes": notes.len(),
        "dropped_silent": notes.len() - kept.len(),
        "chosen": best.how,
        "moves": best.moves,
        "after": compare(&best.y, &reference)?,
        "alternatives": alternatives,
    }))
}
