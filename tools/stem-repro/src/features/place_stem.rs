use std::path::Path;

use anyhow::Result;
use serde_json::{Value, json};

use crate::shared::placement::{compare, place};
use crate::shared::signal::{Format, load, write};
use crate::shared::song::Song;

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
