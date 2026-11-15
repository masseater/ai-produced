use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};
use audio::{Decoded, decode, mean, percentile, round};
use clap::Parser;
use ebur128::{EbuR128, Mode};
use ndarray::Array1;
use num_traits::ToPrimitive;
use serde::Serialize;
use serde_json::{Value, json};
use sonara::core::audio::resample;
use sonara::core::convert::fft_frequencies;
use sonara::core::spectrum::stft_power;
use sonara::feature::spectral::{rms, spectral_centroid};
use sonara::onset::onset_detect;
use sonara::{PadMode, WindowSpec};

const REFS: &str = include_str!("../../../../docs/wiki/data/taste-refs-v1.json");
const SR: u32 = 44100;
const BANDS: [(&str, f32, f32); 5] = [
    ("sub", 20.0, 60.0),
    ("low", 60.0, 250.0),
    ("mid", 250.0, 2000.0),
    ("high_mid", 2000.0, 6000.0),
    ("high", 6000.0, 22050.0),
];
const KEYS: [&str; 13] = [
    "bpm",
    "lufs_integrated",
    "lra",
    "crest_db",
    "stereo_side_to_mid_db",
    "onsets_per_s",
    "spectral_centroid_hz",
    "loudness_p90_minus_p10_db",
    "sub",
    "low",
    "mid",
    "high_mid",
    "high",
];
const MARGIN: f64 = 0.25;

#[derive(Parser)]
#[command(
    about = "Measure a rendered mix with the taste-band v1 definitions and judge it against the band"
)]
struct Args {
    wav: PathBuf,
    #[arg(long)]
    bpm: Option<f64>,
    #[arg(long, default_value_t = 0.0)]
    offset: f64,
    #[arg(long)]
    duration: Option<f64>,
    #[arg(long)]
    judge: bool,
}

#[derive(Serialize)]
struct Metrics {
    bpm: Option<f64>,
    lufs_integrated: Option<f64>,
    lra: Option<f64>,
    true_peak_dbtp: Option<f64>,
    crest_db: f64,
    stereo_side_to_mid_db: f64,
    onsets_per_s: f64,
    spectral_centroid_hz: f64,
    loudness_p90_minus_p10_db: Option<f64>,
    rms_dbfs: f64,
    sub: f64,
    low: f64,
    mid: f64,
    high_mid: f64,
    high: f64,
}

struct Loudness {
    integrated: f64,
    range: f64,
    true_peak: f64,
}

fn loudness(decoded: &Decoded) -> Result<Loudness> {
    let channels = u32::try_from(decoded.channels)?;
    let mut meter = EbuR128::new(
        channels,
        decoded.rate,
        Mode::I | Mode::LRA | Mode::TRUE_PEAK,
    )?;
    meter.add_frames_f32(&decoded.samples)?;
    let peak = (0..channels)
        .map(|channel| meter.true_peak(channel))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .fold(0.0, f64::max);
    Ok(Loudness {
        integrated: round(meter.loudness_global()?, 1),
        range: round(meter.loudness_range()?, 1),
        true_peak: round(20.0 * peak.log10(), 1),
    })
}

fn energy(values: &Array1<f32>) -> Result<f64> {
    mean(values.iter().map(|v| f64::from(*v).powi(2)))
}

fn measure(path: &Path, bpm: Option<f64>, offset: f64, duration: Option<f64>) -> Result<Metrics> {
    let decoded = decode(path)?;
    let [left, right] = decoded.stereo(SR, offset, duration)?;
    let y = (&left + &right) / 2.0;
    let whole = offset == 0.0 && duration.is_none();
    let level = if whole {
        Some(loudness(&decoded)?)
    } else {
        None
    };
    let window = WindowSpec::Named("hann".into());
    let power = stft_power(y.view(), 4096, 2048, &window, true, PadMode::Constant, 2.0)?;
    let freqs = fft_frequencies(SR.to_f32().context("sample rate")?, 4096);
    let band_power = |lo: f32, hi: f32| -> f64 {
        freqs
            .iter()
            .enumerate()
            .filter(|(_, f)| **f >= lo && **f < hi)
            .map(|(bin, _)| power.row(bin).iter().map(|v| f64::from(*v)).sum::<f64>())
            .sum()
    };
    let total = band_power(20.0, f32::INFINITY);
    let mid = (&left + &right) / 2.0;
    let side = (&left - &right) / 2.0;
    let rms_db: Vec<f64> = rms(Some(y.view()), None, 2048, 1024)?
        .row(0)
        .iter()
        .map(|v| 20.0 * (f64::from(*v) + 1e-9).log10())
        .collect();
    let voiced: Vec<f64> = rms_db.iter().copied().filter(|v| *v > -80.0).collect();
    let y22 = resample(y.view(), SR, 22050)?;
    let onsets = onset_detect(Some(y22.view()), None, 22050, 512, false, 0.07, 0)?;
    let seconds = y.len().to_f64().context("sample count")? / f64::from(SR);
    let peak = left
        .iter()
        .chain(right.iter())
        .fold(0.0f64, |m, v| m.max(f64::from(v.abs())));
    let centroid = spectral_centroid(Some(y22.view()), None, 22050.0, 2048, 512)?;
    let bands: Vec<f64> = BANDS
        .iter()
        .map(|(_, lo, hi)| round(10.0 * (band_power(*lo, *hi) / total).log10(), 2))
        .collect();
    let loudness_spread = if voiced.is_empty() {
        None
    } else {
        Some(round(
            percentile(&voiced, 90.0)? - percentile(&voiced, 10.0)?,
            2,
        ))
    };
    Ok(Metrics {
        bpm,
        lufs_integrated: level.as_ref().map(|l| l.integrated),
        lra: level.as_ref().map(|l| l.range),
        true_peak_dbtp: level.as_ref().map(|l| l.true_peak),
        crest_db: round(20.0 * (peak / energy(&y)?.sqrt()).log10(), 2),
        stereo_side_to_mid_db: round(10.0 * (energy(&side)? / energy(&mid)? + 1e-12).log10(), 2),
        onsets_per_s: round(onsets.len().to_f64().context("onset count")? / seconds, 2),
        spectral_centroid_hz: round(mean(centroid.row(0).iter().map(|v| f64::from(*v)))?, 1),
        loudness_p90_minus_p10_db: loudness_spread,
        rms_dbfs: round(10.0 * (energy(&y)? + 1e-12).log10(), 2),
        sub: bands[0],
        low: bands[1],
        mid: bands[2],
        high_mid: bands[3],
        high: bands[4],
    })
}

fn ffmpeg(args: &[&str]) -> Result<()> {
    let status = Command::new("ffmpeg")
        .args(["-loglevel", "error", "-y"])
        .args(args)
        .status()
        .context("ffmpeg")?;
    if !status.success() {
        bail!("ffmpeg {} failed: {status}", args.join(" "));
    }
    Ok(())
}

fn encoded(path: &Path, workdir: &Path) -> Result<PathBuf> {
    let opus = workdir.join("encoded.opus");
    let out = workdir.join("encoded.wav");
    let source = path.to_string_lossy();
    let opus_name = opus.to_string_lossy();
    ffmpeg(&["-i", &source, "-c:a", "libopus", "-b:a", "160k", &opus_name])?;
    ffmpeg(&["-i", &opus_name, "-ar", "48000", &out.to_string_lossy()])?;
    Ok(out)
}

fn band() -> Result<Vec<(&'static str, f64, f64)>> {
    let refs: Value = serde_json::from_str(REFS)?;
    let tracks = refs["tracks"]
        .as_array()
        .context("taste refs without tracks")?;
    KEYS.iter()
        .map(|key| {
            let values: Vec<f64> = tracks
                .iter()
                .map(|t| t[key].as_f64().with_context(|| format!("{key} missing")))
                .collect::<Result<_>>()?;
            let lo = values.iter().copied().fold(f64::INFINITY, f64::min);
            let hi = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            let margin = (hi - lo) * MARGIN;
            Ok((*key, lo - margin, hi + margin))
        })
        .collect()
}

fn judge(metrics: &Value) -> Result<Vec<Value>> {
    Ok(band()?
        .into_iter()
        .map(|(key, lo, hi)| {
            let value = metrics[key].as_f64();
            let in_band = value.is_some_and(|v| lo <= v && v <= hi);
            json!({"key": key, "value": value, "lo": round(lo, 2), "hi": round(hi, 2), "in_band": in_band})
        })
        .collect())
}

fn pretty(value: &Value) -> Result<String> {
    let mut text = Vec::new();
    let formatter = serde_json::ser::PrettyFormatter::with_indent(b" ");
    let mut serializer = serde_json::Serializer::with_formatter(&mut text, formatter);
    value.serialize(&mut serializer)?;
    Ok(String::from_utf8(text)?)
}

fn main() -> Result<()> {
    let args = Args::parse();
    let raw = measure(&args.wav, args.bpm, args.offset, args.duration)?;
    if !args.judge {
        println!("{}", serde_json::to_string(&raw)?);
        return Ok(());
    }
    let workdir = tempfile::tempdir()?;
    let coded = serde_json::to_value(measure(
        &encoded(&args.wav, workdir.path())?,
        args.bpm,
        0.0,
        None,
    )?)?;
    let rows = judge(&coded)?;
    let out_of_band: Vec<Value> = rows
        .iter()
        .filter(|r| r["in_band"] == false)
        .map(|r| r["key"].clone())
        .collect();
    let verdict = if out_of_band.len() <= 2 {
        "in_taste"
    } else {
        "warning"
    };
    let peak = raw.true_peak_dbtp.unwrap_or(f64::INFINITY);
    let report = json!({
        "raw": raw,
        "encoded_opus160": coded,
        "band": rows,
        "out_of_band": out_of_band,
        "verdict": verdict,
        "gates": {
            "true_peak_dbtp<=-1.0": peak <= -1.0,
            "no_clip": peak < 0.0,
            "stereo_not_cancelled": raw.stereo_side_to_mid_db < 0.0,
        },
    });
    println!("{}", pretty(&report)?);
    Ok(())
}
