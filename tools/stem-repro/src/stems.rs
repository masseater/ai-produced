use anyhow::{Context, Result};
use num_traits::ToPrimitive;
use serde_json::{Map, Value, json};

use crate::signal::{RATE, db, hanning, lufs, mono, peak, rfft, rfftfreq, rms, round, side_mid_db};
use crate::song::Song;

const BAND_EDGES: [f64; 11] = [
    20.0, 60.0, 120.0, 250.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0, 16000.0, 22050.0,
];

fn bands(x: &[f64]) -> Result<Value> {
    let window = hanning(x.len())?;
    let windowed: Vec<f64> = x.iter().zip(&window).map(|(v, w)| v * w).collect();
    let power: Vec<f64> = rfft(&windowed, x.len())
        .iter()
        .map(rustfft::num_complex::Complex::norm_sqr)
        .collect();
    let freqs = rfftfreq(x.len())?;
    let total = power.iter().sum::<f64>() + 1e-20;
    let mut out = Map::new();
    for e in BAND_EDGES.windows(2) {
        let band: f64 = power
            .iter()
            .zip(&freqs)
            .filter(|(_, f)| **f >= e[0] && **f < e[1])
            .map(|(p, _)| p)
            .sum();
        out.insert(
            format!("{}-{}", e[0], e[1]),
            json!(round(10.0 * (band / total + 1e-12).log10(), 1)),
        );
    }
    Ok(Value::Object(out))
}

fn report(song: &Song, name: &str) -> Result<Value> {
    let x = song.stem(name, None)?;
    let m = mono(&x);
    let step = (song.bar * RATE).to_usize().context("bar length")?;
    let bar_db: Vec<f64> = m.chunks(step).map(|c| db(rms(c) + 1e-9)).collect();
    let active: Vec<f64> = m
        .chunks(step)
        .zip(&bar_db)
        .filter(|(_, v)| **v > -45.0)
        .flat_map(|(c, _)| c.iter().copied())
        .collect();
    let mut segment = if active.is_empty() { m } else { active };
    segment.truncate(44_100 * 30);
    let p = peak(&x);
    let activity: String = bar_db
        .iter()
        .map(|v| {
            if *v > -45.0 {
                '#'
            } else if *v > -60.0 {
                '+'
            } else {
                '.'
            }
        })
        .collect();
    Ok(json!({
        "lufs": lufs(&x),
        "peak_db": round(db(p + 1e-12), 1),
        "crest_db": round(db(p / (rms(&segment) + 1e-12)), 1),
        "side_mid_db": side_mid_db(&x),
        "activity": activity,
        "bands": bands(&segment)?,
    }))
}

pub fn run(song: &Song) -> Result<Value> {
    let mut out = Map::new();
    for name in &song.stem_names {
        out.insert(name.clone(), report(song, name)?);
    }
    Ok(Value::Object(out))
}
