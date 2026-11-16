use std::path::Path;

use anyhow::{Context, Result};
use indexmap::IndexMap;
use num_traits::ToPrimitive;
use serde_json::{Map, Value, json};

use crate::features::{band_power, centers};
use crate::signal::{
    Frame, RATE, butter, load, lr_db, lufs, mean, mean_square, mono, rms, round, side_mid_db,
    sosfilt, sum,
};
use crate::song::Song;

const SILENT: f64 = 0.001;

type Stems = IndexMap<String, Vec<Frame>>;

fn stems_of(song: &Song, repro: Option<&Path>) -> Result<Stems> {
    let n = song.frames()?;
    song.stem_names
        .iter()
        .map(|name| {
            let x = match repro {
                None => song.stem(name, Some(n))?,
                Some(dir) => load(&dir.join(format!("{name}.wav")), Some(n))?,
            };
            Ok((name.clone(), x))
        })
        .collect()
}

fn band_owner(song: &Song, stems: &Stems, section: &str) -> Result<Value> {
    let mut powers = IndexMap::new();
    for (name, x) in stems {
        powers.insert(name.as_str(), band_power(&mono(song.section(x, section)?))?);
    }
    let rows: Vec<Value> = centers()
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let total = powers.values().map(|p| p[i]).sum::<f64>() + 1e-20;
            let mut shares: Vec<(f64, &str)> =
                powers.iter().map(|(n, p)| (p[i] / total, *n)).collect();
            shares.sort_by(|a, b| b.0.total_cmp(&a.0).then(b.1.cmp(a.1)));
            let top: Vec<Value> = shares
                .iter()
                .take(3)
                .map(|(v, n)| json!([n, (100.0 * v).round_ties_even()]))
                .collect();
            Ok(json!([c.to_i64().context("band center")?, top]))
        })
        .collect::<Result<_>>()?;
    Ok(Value::Array(rows))
}

fn beat_phase(song: &Song, stems: &Stems, section: &str) -> Result<Value> {
    let bins = 48;
    let beat = (song.beat * RATE).to_usize().context("beat length")?;
    let hop = beat / bins;
    let mut out = Map::new();
    for (name, x) in stems {
        let m = mono(song.section(x, section)?);
        if rms(&m) < SILENT {
            out.insert(name.clone(), Value::Null);
            continue;
        }
        let mut acc = vec![0.0; bins];
        let mut k = 0;
        for b in 0..(m.len() / beat).saturating_sub(1) {
            for (j, a) in acc.iter_mut().enumerate() {
                let start = b * beat + j * hop;
                *a += mean_square(&m[start..start + hop]);
            }
            k += 1;
        }
        let e: Vec<f64> = acc
            .iter()
            .map(|a| 10.0 * (a / f64::from(k.max(1)) + 1e-20).log10())
            .collect();
        let top = e.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let values: Vec<f64> = e.iter().step_by(4).map(|v| round(v - top, 1)).collect();
        out.insert(name.clone(), json!(values));
    }
    Ok(Value::Object(out))
}

fn correlation(a: &[f64], b: &[f64]) -> Result<f64> {
    let ma = mean(a.iter().copied())?;
    let mb = mean(b.iter().copied())?;
    let (mut ab, mut aa, mut bb) = (0.0, 0.0, 0.0);
    for (x, y) in a.iter().zip(b) {
        ab += (x - ma) * (y - mb);
        aa += (x - ma) * (x - ma);
        bb += (y - mb) * (y - mb);
    }
    Ok(ab / (aa * bb).sqrt())
}

fn low_overlap(song: &Song, stems: &Stems, section: &str, kick: &str, bass: &str) -> Result<Value> {
    let sos = butter(4, 150.0 / (RATE / 2.0), false);
    let hop = 441;
    let energy = |name: &str| -> Result<Vec<f64>> {
        let x = stems
            .get(name)
            .ok_or_else(|| anyhow::anyhow!("no stem {name}"))?;
        let y = sosfilt(&sos, &mono(song.section(x, section)?));
        Ok((0..y.len().saturating_sub(hop))
            .step_by(hop)
            .map(|i| mean_square(&y[i..i + hop]))
            .collect())
    };
    let (ke, be) = (energy(kick)?, energy(bass)?);
    let log = |e: &[f64]| -> Vec<f64> { e.iter().map(|v| 10.0 * (v + 1e-12).log10()).collect() };
    let (ks, bs) = (ke.iter().sum::<f64>(), be.iter().sum::<f64>());
    Ok(json!({
        "env_corr": round(correlation(&log(&ke), &log(&be))?, 2),
        "kick_share_pct": round(100.0 * ks / (ks + bs), 1),
    }))
}

fn stereo(song: &Song, stems: &Stems, section: &str) -> Result<Value> {
    let mut out = Map::new();
    for (name, x) in stems {
        let segment = song.section(x, section)?;
        let all: Vec<f64> = segment.iter().flatten().copied().collect();
        let segment = if rms(&all) > SILENT { segment } else { x };
        out.insert(
            name.clone(),
            json!({"side_mid_db": side_mid_db(segment), "lr_db": lr_db(segment)}),
        );
    }
    Ok(Value::Object(out))
}

fn section_balance(song: &Song, stems: &Stems) -> Result<Value> {
    let mut out = Map::new();
    for section in song.sections.keys() {
        let segments: Vec<(&String, Vec<Frame>)> = stems
            .iter()
            .map(|(n, x)| Ok((n, song.section(x, section)?.to_vec())))
            .collect::<Result<_>>()?;
        let all: Vec<Vec<Frame>> = segments.iter().map(|(_, s)| s.clone()).collect();
        let mix = lufs(&sum(&all));
        let mut row = Map::new();
        row.insert("mix_lufs".into(), json!(mix));
        for (name, segment) in &segments {
            let value = match (lufs(segment), mix) {
                (Some(v), Some(m)) if v >= -70.0 => json!(round(v - m, 1)),
                _ => Value::Null,
            };
            row.insert((*name).clone(), value);
        }
        out.insert(section.clone(), Value::Object(row));
    }
    Ok(Value::Object(out))
}

fn analyze(song: &Song, stems: &Stems, section: &str, kick: &str, bass: &str) -> Result<Value> {
    Ok(json!({
        "band_owner": band_owner(song, stems, section)?,
        "beat_phase_db": beat_phase(song, stems, section)?,
        "low_kick_vs_bass": low_overlap(song, stems, section, kick, bass)?,
        "stereo": stereo(song, stems, section)?,
        "section_balance_rel_mix": section_balance(song, stems)?,
    }))
}

pub fn run(
    song: &Song,
    section: &str,
    kick: &str,
    bass: &str,
    repro: Option<&Path>,
) -> Result<Value> {
    let mut out = Map::new();
    out.insert(
        "orig".into(),
        analyze(song, &stems_of(song, None)?, section, kick, bass)?,
    );
    if let Some(dir) = repro {
        out.insert(
            "repro".into(),
            analyze(song, &stems_of(song, Some(dir))?, section, kick, bass)?,
        );
    }
    Ok(Value::Object(out))
}
