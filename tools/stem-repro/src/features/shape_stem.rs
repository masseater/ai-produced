use std::path::Path;

use anyhow::Result;
use serde_json::{Value, json};

use crate::shared::placement::{compare, place};
use crate::shared::shaping::{bar_gain, beat_gain};
use crate::shared::signal::{Format, load, write};
use crate::shared::song::Song;

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
