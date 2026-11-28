use std::path::Path;

use anyhow::{Context, Result};
use ndarray::ArrayView1;
use num_traits::ToPrimitive;
use serde_json::{Value, json};
use sonara::core::constantq::cqt;

use crate::shared::notes::{Note, read, write};
use crate::shared::signal::{SR, mono, percentile};
use crate::shared::song::Song;

fn midi_hz(pitch: f64) -> f64 {
    440.0 * ((pitch - 69.0) / 12.0).exp2()
}

fn supported(song: &Song, stem: &str, notes: &[Note], threshold: f64) -> Result<Vec<Note>> {
    let hop = 512;
    let m: Vec<f32> = mono(&song.stem(stem, None)?)
        .iter()
        .map(|v| v.to_f32().context("sample"))
        .collect::<Result<_>>()?;
    let c = cqt(
        ArrayView1::from(&m),
        SR,
        hop,
        Some(midi_hz(24.0).to_f32().context("lowest pitch")?),
        84,
        12,
        1.0,
    )?;
    let levels = c.mapv(|v| 20.0 * (f64::from(v.norm()) + 1e-9).log10());
    let frames = levels.ncols();
    let tops: Vec<f64> = (0..frames)
        .map(|t| {
            levels
                .column(t)
                .iter()
                .copied()
                .fold(f64::NEG_INFINITY, f64::max)
        })
        .collect();
    let reference = percentile(&tops, 99.0)?;
    let mut kept = Vec::new();
    for note in notes {
        let i = ((note.start + 0.03) * f64::from(SR) / hop.to_f64().context("hop")?)
            .max(0.0)
            .to_usize()
            .context("note start")?;
        let Some(bin) = usize::from(note.pitch).checked_sub(24) else {
            continue;
        };
        if bin >= 84 || i >= frames {
            continue;
        }
        let level = (i..(i + 4).min(frames))
            .map(|t| levels[(bin, t)])
            .fold(f64::NEG_INFINITY, f64::max)
            - reference;
        if level > threshold {
            kept.push(Note {
                velocity: (127.0 + 2.5 * level)
                    .clamp(30.0, 127.0)
                    .to_u8()
                    .context("velocity")?,
                ..*note
            });
        }
    }
    Ok(kept)
}

fn tuples(notes: &[Note]) -> Value {
    json!(
        notes
            .iter()
            .map(|n| json!([n.start, n.end, n.pitch, n.velocity]))
            .collect::<Vec<_>>()
    )
}

pub fn run(
    song: &Song,
    stem: &str,
    midi: &Path,
    bars: Option<[f64; 2]>,
    threshold: f64,
) -> Result<Value> {
    let mut notes = read(midi, None)?;
    if let Some([b0, b1]) = bars {
        notes = song.notes_in(&notes, b0, b1);
    }
    let kept = supported(song, stem, &notes, threshold)?;
    Ok(json!({"stem": stem, "input": notes.len(), "kept": kept.len(), "notes": tuples(&kept)}))
}

fn splice(notes: &[Note], other: &[Note], ranges: &[(f64, f64)]) -> Vec<Note> {
    let inside = |t: f64| ranges.iter().any(|(a, b)| *a <= t && t < *b);
    let mut out: Vec<Note> = notes
        .iter()
        .filter(|n| !inside(n.start))
        .chain(other.iter().filter(|n| inside(n.start)))
        .copied()
        .collect();
    sort(&mut out);
    out
}

fn sort(notes: &mut [Note]) {
    notes.sort_by(|a, b| {
        (a.start, a.end, a.pitch, a.velocity)
            .partial_cmp(&(b.start, b.end, b.pitch, b.velocity))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
}

fn grid(notes: &[Note], step: f64, fixed: bool) -> Result<Vec<Note>> {
    let mut best: Vec<((i64, u8), Note)> = Vec::new();
    for n in notes {
        let g = (n.start / step).round_ties_even();
        let t = if (n.start / step - g).abs() <= 0.3 {
            g * step
        } else {
            n.start
        };
        let key = (
            (t * 10_000.0)
                .round_ties_even()
                .to_i64()
                .context("note start")?,
            n.pitch,
        );
        let candidate = Note {
            start: t,
            end: if fixed { t + step } else { n.end.max(t + 0.03) },
            ..*n
        };
        match best.iter_mut().find(|(k, _)| *k == key) {
            Some((_, existing)) if n.velocity > existing.velocity => *existing = candidate,
            Some(_) => {}
            None => best.push((key, candidate)),
        }
    }
    let mut out: Vec<Note> = best.into_iter().map(|(_, n)| n).collect();
    sort(&mut out);
    Ok(out)
}

pub struct Edit<'a> {
    pub midi: &'a Path,
    pub track: Option<usize>,
    pub out: &'a Path,
    pub splice: Option<&'a Path>,
    pub splice_bars: &'a [f64],
    pub grid: Option<u32>,
    pub fixed: bool,
}

pub fn run_edit(song: &Song, edit: &Edit) -> Result<Value> {
    let mut notes = read(edit.midi, edit.track)?;
    if let Some(other) = edit.splice {
        let ranges: Vec<(f64, f64)> = edit
            .splice_bars
            .as_chunks::<2>()
            .0
            .iter()
            .map(|[a, b]| (a * song.bar, b * song.bar))
            .collect();
        notes = splice(&notes, &read(other, None)?, &ranges);
    }
    if let Some(division) = edit.grid {
        notes = grid(&notes, song.bar / f64::from(division), edit.fixed)?;
    }
    write(edit.out, &notes, song.bpm)?;
    Ok(json!({"notes": notes.len(), "out": edit.out}))
}
