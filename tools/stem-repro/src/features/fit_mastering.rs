use std::path::Path;

use anyhow::{Context, Result};
use serde_json::{Map, Value, json};

use crate::shared::eq::{apply, curve};
use crate::shared::placement::compare;
use crate::shared::signal::{
    Format, Frame, RATE, channel, correlate_full, db, join, load, lufs, mono, percentile,
    resample_poly, round, scale, stats, sum, write,
};
use crate::shared::song::Song;
use crate::shared::spectral::env;

fn true_peak_db(x: &[Frame]) -> Result<f64> {
    let mut peak = 0.0f64;
    for c in 0..2 {
        peak = resample_poly(&channel(x, c), 4, 1)?
            .iter()
            .fold(peak, |a, v| a.max(v.abs()));
    }
    Ok(round(db(peak + 1e-12), 2))
}

fn align(a: &[Frame], b: &[Frame]) -> Result<i64> {
    let max_lag = 44_100 / 10;
    let limit = 44_100 * 30;
    let ma: Vec<f64> = mono(&a[..a.len().min(limit)]);
    let mb: Vec<f64> = mono(&b[..b.len().min(limit)]);
    let c = correlate_full(&mb, &ma)?;
    let mid = ma.len() - 1;
    let window = &c[mid.saturating_sub(max_lag)..(mid + max_lag + 1).min(c.len())];
    let best = window
        .iter()
        .enumerate()
        .max_by(|x, y| x.1.total_cmp(y.1).then(y.0.cmp(&x.0)))
        .map_or(0, |(i, _)| i);
    Ok(i64::try_from(best)? - i64::try_from(max_lag)?)
}

fn median(x: &[f64]) -> Result<f64> {
    percentile(x, 50.0)
}

fn transfer(src: &[Frame], dst: &[Frame]) -> Result<Vec<Value>> {
    let (hop, step) = (441, 3.0);
    let (es, ed) = (env(&mono(src), hop)?, env(&mono(dst), hop)?);
    let n = es.len().min(ed.len());
    let top = percentile(&es[..n], 99.9)?;
    let mut rows = Vec::new();
    let mut lo = top - 30.0;
    while lo < top {
        let diffs: Vec<f64> = (0..n)
            .filter(|i| es[*i] >= lo && es[*i] < lo + step)
            .map(|i| ed[i] - es[i])
            .collect();
        if diffs.len() > 20 {
            rows.push(json!({
                "in_db_rel_top": round(lo - top, 1),
                "gain_db": round(median(&diffs)?, 2),
                "frames": diffs.len(),
            }));
        }
        lo += step;
    }
    Ok(rows)
}

fn transfer_gains(src: &[Frame], dst: &[Frame]) -> Result<Vec<f64>> {
    Ok(transfer(src, dst)?
        .iter()
        .filter_map(|r| r["gain_db"].as_f64())
        .collect())
}

fn peak_reduction(src: &[Frame], dst: &[Frame]) -> Result<Value> {
    let hop = 441;
    let peaks = |x: &[Frame]| -> Vec<f64> {
        let m: Vec<f64> = mono(x).iter().map(|v| v.abs()).collect();
        (0..m.len().saturating_sub(hop))
            .step_by(hop)
            .map(|i| m[i..i + hop].iter().copied().fold(0.0, f64::max))
            .collect()
    };
    let (ps, pd) = (peaks(src), peaks(dst));
    let abs = |x: &[Frame]| -> Vec<f64> { x.iter().flatten().map(|v| v.abs()).collect() };
    let g = db((median(&abs(dst))? + 1e-12) / (median(&abs(src))? + 1e-12));
    let threshold = percentile(&ps, 99.0)?;
    let reduction: Vec<f64> = ps
        .iter()
        .zip(&pd)
        .filter(|(s, _)| **s > threshold)
        .map(|(s, d)| g - db((d + 1e-12) / (s + 1e-12)))
        .collect();
    Ok(json!({
        "median_gain_db": round(g, 2),
        "loudest_1pct_extra_reduction_db": {
            "p50": round(median(&reduction)?, 2),
            "max": round(reduction.iter().copied().fold(f64::NEG_INFINITY, f64::max), 2),
        },
    }))
}

fn measure(src: &[Frame], dst: &[Frame]) -> Result<Value> {
    let lag = align(src, dst)?;
    let skip = usize::try_from(lag.unsigned_abs())?;
    let (mut src, mut dst) = (src, dst);
    if lag > 0 {
        dst = &dst[skip..];
    } else if lag < 0 {
        src = &src[skip..];
    }
    let n = src.len().min(dst.len());
    let (src, dst) = (&src[..n], &dst[..n]);
    let (from, to) = (stats(src), stats(dst));
    let gain = match (from.lufs, to.lufs) {
        (Some(a), Some(b)) => json!(round(b - a, 1)),
        _ => Value::Null,
    };
    let mut src_stats = serde_json::to_value(&from).unwrap_or(Value::Null);
    src_stats["true_peak_db"] = json!(true_peak_db(src)?);
    let mut dst_stats = serde_json::to_value(&to).unwrap_or(Value::Null);
    dst_stats["true_peak_db"] = json!(true_peak_db(dst)?);
    Ok(json!({
        "lag_samples": lag,
        "src": src_stats,
        "dst": dst_stats,
        "loudness_gain_db": gain,
        "eq_tilt_db": curve(src, dst)?.iter().map(|v| round(*v, 1)).collect::<Vec<_>>(),
        "level_transfer": transfer(src, dst)?,
        "peaks": peak_reduction(src, dst)?,
    }))
}

pub fn run(song: &Song, bus: bool) -> Result<Value> {
    let n = song.frames()?;
    let premaster = load(&song.premaster, Some(n))?;
    let mut out = Map::new();
    if bus {
        let stems: Vec<Vec<Frame>> = song
            .stem_names
            .iter()
            .map(|name| song.stem(name, Some(n)))
            .collect::<Result<_>>()?;
        out.insert(
            "bus_stem_sum_to_premaster".into(),
            measure(&sum(&stems), &premaster)?,
        );
    }
    if let Some(master) = &song.master {
        out.insert(
            "mastering_premaster_to_master".into(),
            measure(&premaster, &load(master, Some(n))?)?,
        );
    }
    Ok(Value::Object(out))
}

fn loudness_to(y: &[Frame], target: f64) -> Vec<Frame> {
    let current = lufs(y).unwrap_or(target);
    scale(y, 10f64.powf((target - current) / 20.0))
}

struct Compressor {
    threshold: f64,
    ratio_inverse: f64,
    attack: f64,
    release: f64,
}

impl Compressor {
    fn new(threshold_db: f64, ratio: f64, attack_ms: f64, release_ms: f64) -> Self {
        let coefficient = |ms: f64| {
            if ms < 1e-3 {
                0.0
            } else {
                (-2.0 * std::f64::consts::PI * 1000.0 / RATE / ms).exp()
            }
        };
        Self {
            threshold: 10f64.powf(threshold_db / 20.0),
            ratio_inverse: 1.0 / ratio,
            attack: coefficient(attack_ms),
            release: coefficient(release_ms),
        }
    }

    fn process(&self, x: &[Frame]) -> Vec<Frame> {
        let mut state = [0.0f64; 2];
        x.iter()
            .map(|frame| {
                let mut out = [0.0; 2];
                for c in 0..2 {
                    let input = frame[c] * frame[c];
                    let k = if input > state[c] {
                        self.attack
                    } else {
                        self.release
                    };
                    state[c] = input + k * (state[c] - input);
                    let level = state[c].sqrt();
                    let gain = if level < self.threshold {
                        1.0
                    } else {
                        (level / self.threshold).powf(self.ratio_inverse - 1.0)
                    };
                    out[c] = gain * frame[c];
                }
                out
            })
            .collect()
    }
}

fn fit_bus(stem_sum: &[Frame], premaster: &[Frame]) -> Result<(Vec<Frame>, Value)> {
    let target = lufs(premaster).context("the premaster is silent")?;
    let want = stats(premaster).crest;
    let x = loudness_to(stem_sum, target);
    let peak = db(crate::shared::signal::peak(&x));
    let mut best: Option<(f64, Value, Vec<Frame>, Value)> = None;
    for below in [3.0, 6.0, 9.0, 12.0, 15.0] {
        for ratio in [2.0, 4.0, 8.0] {
            for attack in [0.5, 3.0, 10.0] {
                for release in [50.0, 150.0] {
                    let compressor = Compressor::new(peak - below, ratio, attack, release);
                    let y = loudness_to(&compressor.process(&x), target);
                    let c = compare(&y, premaster)?;
                    let score = (c.cand.crest - want).abs() + c.env_db / 2.0;
                    if best.as_ref().is_none_or(|b| score < b.0) {
                        best = Some((
                            score,
                            json!({"threshold_below_peak_db": below, "ratio": ratio, "attack_ms": attack, "release_ms": release}),
                            y,
                            serde_json::to_value(&c)?,
                        ));
                    }
                }
            }
        }
    }
    let (_, compressor, y, versus) = best.context("no compressor setting was tried")?;
    let peaks = peak_reduction(&x, &y)?;
    Ok((
        y,
        json!({"compressor": compressor, "peaks": peaks, "vs_premaster": versus}),
    ))
}

fn soft_clip(y: &[Frame], ceiling_db: f64, knee: f64, oversample: usize) -> Result<Vec<Frame>> {
    let ceiling = 10f64.powf(ceiling_db / 20.0);
    let channels: Vec<Vec<f64>> = (0..2)
        .map(|ch| {
            let input = channel(y, ch);
            let up = if oversample > 1 {
                resample_poly(&input, oversample, 1)?
            } else {
                input
            };
            let clipped: Vec<f64> = up
                .iter()
                .map(|sample| {
                    let ratio = sample / ceiling;
                    ratio / (1.0 + ratio.abs().powf(knee)).powf(1.0 / knee) * ceiling
                })
                .collect();
            let mut back = if oversample > 1 {
                resample_poly(&clipped, 1, oversample)?
            } else {
                clipped
            };
            back.truncate(y.len());
            Ok(back)
        })
        .collect::<Result<_>>()?;
    Ok(join(&channels[0], &channels[1]))
}

fn fit_master(mix: &[Frame], premaster: &[Frame], master: &[Frame]) -> Result<(Vec<Frame>, Value)> {
    let target = lufs(master).context("the master is silent")?;
    let pre = apply(mix, &curve(premaster, master)?)?;
    let want = transfer_gains(premaster, master)?;
    let (want_crest, want_tp) = (stats(master).crest, true_peak_db(master)?);
    let mut best: Option<(f64, Value, Vec<Frame>)> = None;
    for ceiling in [-3.0, -2.0, -1.5, -1.0, -0.5, 0.0] {
        for knee in [4.0, 8.0, 32.0] {
            for oversample in [1, 4] {
                let mut g = target - lufs(&pre).unwrap_or(target);
                let mut y = Vec::new();
                for _ in 0..5 {
                    y = soft_clip(
                        &scale(&pre, 10f64.powf(g / 20.0)),
                        ceiling,
                        knee,
                        oversample,
                    )?;
                    g += target - lufs(&y).unwrap_or(target);
                }
                let got = transfer_gains(mix, &y)?;
                let worst = got
                    .iter()
                    .zip(&want)
                    .map(|(a, b)| (a - b).abs())
                    .fold(0.0, f64::max);
                let score = (stats(&y).crest - want_crest).abs()
                    + worst / 2.0
                    + (true_peak_db(&y)? - want_tp).abs() / 2.0;
                if best.as_ref().is_none_or(|b| score < b.0) {
                    best = Some((
                        score,
                        json!({"ceiling_db": ceiling, "knee": knee, "oversample": oversample, "gain_db": round(g, 1)}),
                        y,
                    ));
                }
            }
        }
    }
    let (_, clipper, y) = best.context("no clipper setting was tried")?;
    let report = json!({
        "clipper": clipper,
        "true_peak_db": true_peak_db(&y)?,
        "level_transfer": transfer(mix, &y)?,
        "vs_master": compare(&y, master)?,
    });
    Ok((y, report))
}

pub fn run_fit(song: &Song, stems_dir: &Path, out: &Path) -> Result<Value> {
    let n = song.frames()?;
    let premaster = load(&song.premaster, Some(n))?;
    let master = load(
        song.master.as_ref().context("song.json has no master")?,
        Some(n),
    )?;
    let mut paths: Vec<_> = std::fs::read_dir(stems_dir)?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "wav"))
        .collect();
    paths.sort();
    let stems: Vec<Vec<Frame>> = paths
        .iter()
        .map(|p| load(p, Some(n)))
        .collect::<Result<_>>()?;
    let (mix, bus) = fit_bus(&sum(&stems), &premaster)?;
    let (mastered, report) = fit_master(&mix, &premaster, &master)?;
    write(&out.join("mix_noMst.wav"), &mix, Format::Float)?;
    write(&out.join("master.wav"), &mastered, Format::Pcm24)?;
    Ok(json!({"bus": bus, "master": report}))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compressor_leaves_quiet_signal_alone_and_reduces_loud_signal() {
        let compressor = Compressor::new(-20.0, 4.0, 1.0, 50.0);
        let quiet: Vec<Frame> = vec![[0.01, 0.01]; 10_000];
        assert!((compressor.process(&quiet)[9_999][0] - 0.01).abs() < 1e-12);
        let loud: Vec<Frame> = vec![[1.0, 1.0]; 10_000];
        let out = compressor.process(&loud)[9_999][0];
        assert!((db(out) - (-20.0 + 20.0 / 4.0)).abs() < 0.01, "{}", db(out));
    }
}
