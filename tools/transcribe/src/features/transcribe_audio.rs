use std::fs;
use std::path::{Path, PathBuf};

use crate::shared::basic_pitch_engine::{BasicPitchParams, basic_pitch};
use crate::shared::midi::{self, Note};
use crate::shared::muscriptor::{Muscriptor, SAMPLE_RATE, instrument_name};
use anyhow::{Context, Result};
use clap::{Parser, ValueEnum};
use num_traits::ToPrimitive;

const SILENCE_DB: f64 = -60.0;

#[derive(Clone, Copy, ValueEnum)]
enum Engine {
    Muscriptor,
    BasicPitch,
}

#[derive(Parser)]
#[command(about = "Transcribe stems or mixes to MIDI with MuScriptor (small, ONNX) or Basic Pitch")]
struct Args {
    #[arg(required = true)]
    inputs: Vec<PathBuf>,
    #[arg(short, long, default_value = ".")]
    out_dir: PathBuf,
    #[arg(short, long, value_enum, default_value_t = Engine::Muscriptor)]
    engine: Engine,
    #[arg(long, default_value_t = 0.5)]
    onset_threshold: f64,
    #[arg(long, default_value_t = 0.3)]
    frame_threshold: f64,
    #[arg(long, default_value_t = 58.0)]
    minimum_note_length: f64,
}

fn sounding(note: &Note, samples: &[f32], rate: u32) -> Result<bool> {
    let rate = f64::from(rate);
    let start = (note.onset * rate).to_usize().context("note onset")?;
    let window = (0.08 * rate).to_usize().context("window")?;
    let chunk = &samples[start.min(samples.len())..(start + window).min(samples.len())];
    if chunk.is_empty() {
        return Ok(false);
    }
    let energy = chunk.iter().map(|v| f64::from(*v).powi(2)).sum::<f64>()
        / chunk.len().to_f64().context("window length")?;
    Ok(10.0 * (energy + 1e-12).log10() > SILENCE_DB)
}

fn muscriptor(model: &Muscriptor, wav: &Path) -> Result<Vec<u8>> {
    let decoded = audio::decode(wav)?;
    let notes = model.transcribe(decoded.mono(SAMPLE_RATE)?.as_slice().unwrap_or_default())?;
    let native = decoded.mono(decoded.rate)?;
    let samples = native.as_slice().unwrap_or_default();
    let mut kept: Vec<Note> = Vec::new();
    for note in notes {
        if sounding(&note, samples, decoded.rate)? {
            kept.push(note);
        }
    }
    midi::write(&kept, instrument_name)
}

pub fn run() -> Result<()> {
    for out in transcribe(&Args::parse())? {
        println!("{}", out.display());
    }
    Ok(())
}

fn transcribe(args: &Args) -> Result<Vec<PathBuf>> {
    fs::create_dir_all(&args.out_dir)?;
    let model = match args.engine {
        Engine::Muscriptor => Some(Muscriptor::load()?),
        Engine::BasicPitch => None,
    };
    let params = BasicPitchParams {
        onset_threshold: args.onset_threshold,
        frame_threshold: args.frame_threshold,
        minimum_note_length_ms: args.minimum_note_length,
    };
    let mut written = Vec::new();
    for wav in &args.inputs {
        let bytes = match &model {
            Some(model) => muscriptor(model, wav)?,
            None => basic_pitch(wav, &params)?,
        };
        let stem = wav.file_stem().with_context(|| wav.display().to_string())?;
        let out = args.out_dir.join(stem).with_extension("mid");
        fs::write(&out, bytes)?;
        written.push(out);
    }
    Ok(written)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::note_matching::{Selection, score_files};
    use crate::shared::test_melody::{write_melody, write_reference};

    fn transcribed_recall(engine: &str) -> f64 {
        let dir = tempfile::tempdir().unwrap();
        let wav = dir.path().join("melody.wav");
        write_melody(&wav);
        let truth = dir.path().join("truth.mid");
        write_reference(&truth, 0.0);
        let out_dir = dir.path().join("out");
        let args = Args::parse_from([
            "transcribe",
            wav.to_str().unwrap(),
            "-o",
            out_dir.to_str().unwrap(),
            "-e",
            engine,
        ]);
        let written = transcribe(&args).unwrap();
        assert_eq!(written, [out_dir.join("melody.mid")]);
        let selection = Selection {
            ref_instrument: None,
            est_programs: None,
            max_shift: 0.1,
        };
        let score = score_files(&truth, &written[0], &selection).unwrap();
        eprintln!("{engine}: {score}");
        score["recall"].as_f64().unwrap()
    }

    #[test]
    fn basic_pitch_finds_every_plucked_note() {
        assert!(transcribed_recall("basic-pitch") >= 0.99);
    }

    #[test]
    #[ignore = "downloads the 400 MB MuScriptor model and decodes for minutes"]
    fn muscriptor_finds_every_plucked_note() {
        assert!(transcribed_recall("muscriptor") >= 0.99);
    }
}
