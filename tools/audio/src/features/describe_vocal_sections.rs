use std::fs;
use std::path::PathBuf;

use crate::shared::audio_file::decode;
use crate::shared::statistics::{mean, round};
use anyhow::{Context, Result};
use clap::Parser;
use ndarray::{ArrayView1, Axis, Slice};
use num_traits::ToPrimitive;
use serde::Deserialize;
use serde_json::{Map, Value, json};
use sonara::core::pitch::pyin;
use sonara::feature::spectral::{rms, spectral_centroid, spectral_flatness};

const SR: u32 = 22050;
const HOP: usize = 256;

#[derive(Parser)]
#[command(
    about = "Per-section vocal description: level, brightness, noisiness, gaps, pitch wobble"
)]
struct Args {
    wav: PathBuf,
    #[arg(long)]
    score: PathBuf,
}

#[derive(Deserialize)]
struct Score {
    bpm: f64,
    sections: Vec<Section>,
}

#[derive(Deserialize)]
struct Section {
    name: String,
    bars: u32,
}

fn describe(y: &ArrayView1<f32>) -> Result<Value> {
    let level: Vec<f64> = rms(Some(y.view()), None, 2048, HOP)?
        .row(0)
        .iter()
        .map(|v| f64::from(*v))
        .collect();
    let threshold = 10f64.powf(-40.0 / 20.0);
    let loud: Vec<bool> = level.iter().map(|v| *v > threshold).collect();
    let loud_count = loud.iter().filter(|l| **l).count();
    if loud_count == 0 {
        return Ok(Value::Null);
    }
    let (f0, voiced_flag, _) = pyin(y.view(), 150.0, 900.0, SR, 2048, Some(HOP))?;
    let voiced: Vec<bool> = voiced_flag
        .iter()
        .zip(&loud)
        .map(|(v, l)| *v && *l)
        .collect();
    let cents: Vec<f64> = voiced
        .iter()
        .enumerate()
        .filter(|(_, v)| **v)
        .map(|(i, _)| 1200.0 * (f64::from(f0[i]) / 440.0).log2())
        .collect();
    let wobble = if cents.len() > 15 {
        let detrended: Vec<f64> = (0..cents.len())
            .map(|i| {
                let lo = i.saturating_sub(7);
                let hi = (i + 8).min(cents.len());
                cents[i] - cents[lo..hi].iter().sum::<f64>() / 15.0
            })
            .collect();
        Some(round(
            mean(detrended[7..detrended.len() - 7].iter().map(|v| v.abs()))?,
            1,
        ))
    } else {
        None
    };
    let at_loud = |values: ArrayView1<f32>| {
        mean(
            values
                .iter()
                .zip(&loud)
                .filter(|(_, l)| **l)
                .map(|(v, _)| f64::from(*v)),
        )
    };
    let centroid = spectral_centroid(
        Some(y.view()),
        None,
        SR.to_f32().context("sample rate")?,
        2048,
        HOP,
    )?;
    let flatness = spectral_flatness(Some(y.view()), None, 2048, HOP, 1e-10, 2.0)?;
    let loud_rms = mean(
        level
            .iter()
            .zip(&loud)
            .filter(|(_, l)| **l)
            .map(|(v, _)| v * v),
    )?;
    let frames = loud.len().to_f64().context("frame count")?;
    let loud_frames = loud_count.to_f64().context("loud frame count")?;
    let voiced_frames = voiced
        .iter()
        .filter(|v| **v)
        .count()
        .to_f64()
        .context("voiced frame count")?;
    Ok(json!({
        "rms_db": round(20.0 * loud_rms.sqrt().log10(), 1),
        "centroid_hz": at_loud(centroid.row(0))?.round_ties_even().to_i64().context("centroid")?,
        "flatness": round(at_loud(flatness.row(0))?, 4),
        "sound_ratio": round(loud_frames / frames, 2),
        "voiced_of_sound": round(voiced_frames / loud_frames, 2),
        "pitch_wobble_cents": wobble,
    }))
}

pub fn run() -> Result<()> {
    println!("{}", report(&Args::parse())?);
    Ok(())
}

fn report(args: &Args) -> Result<String> {
    let text = fs::read_to_string(&args.score).with_context(|| args.score.display().to_string())?;
    let score: Score = toml::from_str(&text)?;
    let y = decode(&args.wav)?.mono(SR)?;
    let seconds_per_bar = 4.0 * 60.0 / score.bpm;
    let rate = f64::from(SR);
    let mut out = Map::new();
    let mut bar = 0;
    for section in &score.sections {
        let start = (f64::from(bar) * seconds_per_bar * rate)
            .to_usize()
            .with_context(|| format!("bad start of {}", section.name))?
            .min(y.len());
        bar += section.bars;
        let end = (f64::from(bar) * seconds_per_bar * rate)
            .to_usize()
            .with_context(|| format!("bad end of {}", section.name))?
            .clamp(start, y.len());
        out.insert(
            section.name.clone(),
            describe(&y.slice_axis(Axis(0), Slice::from(start..end)))?,
        );
    }
    let mut text = Vec::new();
    let formatter = serde_json::ser::PrettyFormatter::with_indent(b" ");
    let mut serializer = serde_json::Serializer::with_formatter(&mut text, formatter);
    serde::Serialize::serialize(&Value::Object(out), &mut serializer)?;
    Ok(String::from_utf8(text)?)
}

#[cfg(test)]
mod tests {
    use std::f64::consts::PI;
    use std::fs;

    use serde_json::Value;

    use super::*;
    use crate::shared::test_signal::{near, write_wav};

    #[test]
    fn skips_silence_and_tracks_a_steady_pitch() {
        let dir = tempfile::tempdir().unwrap();
        let rate = 22050;
        let path = dir.path().join("voice.wav");
        let bar = 2.0;
        let frames = (f64::from(rate) * bar * 4.0).to_u32().unwrap();
        let voice: Vec<f64> = (0..frames)
            .map(|i| {
                let t = f64::from(i) / f64::from(rate);
                if t < bar * 2.0 {
                    0.0
                } else {
                    0.3 * (2.0 * PI * 440.0 * t).sin()
                }
            })
            .collect();
        write_wav(&path, rate, &[voice]);
        let score = dir.path().join("score.toml");
        fs::write(
            &score,
            "bpm = 120\n[[sections]]\nname = \"Rest\"\nbars = 2\n[[sections]]\nname = \"Sing\"\nbars = 2\n",
        )
        .unwrap();
        let args = Args::parse_from([
            "vocal-sections",
            path.to_str().unwrap(),
            "--score",
            score.to_str().unwrap(),
        ]);
        let result: Value = serde_json::from_str(&report(&args).unwrap()).unwrap();
        assert!(result["Rest"].is_null(), "{result}");
        let sing = &result["Sing"];
        near(&sing["sound_ratio"], 1.0, 0.02);
        near(&sing["voiced_of_sound"], 1.0, 0.05);
        near(&sing["centroid_hz"], 440.0, 40.0);
        near(&sing["pitch_wobble_cents"], 0.0, 2.0);
        near(&sing["rms_db"], -13.5, 0.2);
    }
}
