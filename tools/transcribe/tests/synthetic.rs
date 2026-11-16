use std::fs;
use std::path::Path;
use std::process::Command;

use num_traits::ToPrimitive;
use serde_json::Value;
use transcribe::midi::{self, Note, read};

const MELODY: [(u8, f64, f64); 7] = [
    (60, 0.5, 1.0),
    (64, 1.0, 1.5),
    (67, 1.5, 2.0),
    (72, 2.0, 2.5),
    (67, 2.5, 3.0),
    (64, 3.0, 3.5),
    (60, 3.5, 4.0),
];

fn hz(pitch: u8) -> f64 {
    440.0 * 2f64.powf((f64::from(pitch) - 69.0) / 12.0)
}

fn write_melody(path: &Path) {
    let rate = 44100u32;
    let frames = 44100usize;
    let mut samples = vec![0f64; frames * 5];
    let mut seed = 1u64;
    for (pitch, onset, _) in MELODY {
        let period = (f64::from(rate) / hz(pitch)).to_usize().unwrap();
        let mut string: Vec<f64> = (0..period)
            .map(|_| {
                seed = seed
                    .wrapping_mul(6_364_136_223_846_793_005)
                    .wrapping_add(1_442_695_040_888_963_407);
                f64::from(u32::try_from(seed >> 33).unwrap()) / f64::from(1u32 << 31) - 1.0
            })
            .collect();
        let start = (onset * f64::from(rate)).to_usize().unwrap();
        for i in 0..frames * 2 {
            let at = i % period;
            let value = string[at];
            string[at] = 0.996 * 0.5 * (value + string[(at + 1) % period]);
            if let Some(sample) = samples.get_mut(start + i) {
                *sample += 0.4 * value;
            }
        }
    }
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(path, spec).unwrap();
    for sample in samples {
        writer
            .write_sample((sample.clamp(-1.0, 1.0) * 32767.0).to_i16().unwrap())
            .unwrap();
    }
    writer.finalize().unwrap();
}

fn reference(path: &Path) {
    let notes: Vec<Note> = MELODY
        .iter()
        .map(|(pitch, onset, offset)| Note {
            program: 0,
            pitch: *pitch,
            onset: *onset,
            offset: *offset,
        })
        .collect();
    fs::write(path, midi::write(&notes, |_| "piano".to_string()).unwrap()).unwrap();
}

fn run(bin: &str, args: &[&str]) -> String {
    let output = Command::new(bin).args(args).output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn midi_round_trips_through_the_reader() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("reference.mid");
    reference(&path);
    let instruments = read(&fs::read(&path).unwrap()).unwrap();
    assert_eq!(instruments.len(), 1);
    let notes = &instruments[0].notes;
    assert_eq!(notes.len(), MELODY.len());
    for (note, (pitch, onset, offset)) in notes.iter().zip(MELODY) {
        assert_eq!(note.pitch, pitch);
        assert!((note.start - onset).abs() < 1e-3);
        assert!((note.end - offset).abs() < 1e-3);
    }
}

#[test]
fn score_is_perfect_against_itself_and_finds_a_shift() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("reference.mid");
    reference(&path);
    let shifted = dir.path().join("shifted.mid");
    let notes: Vec<Note> = MELODY
        .iter()
        .map(|(pitch, onset, offset)| Note {
            program: 0,
            pitch: *pitch,
            onset: onset + 0.08,
            offset: offset + 0.08,
        })
        .collect();
    fs::write(
        &shifted,
        midi::write(&notes, |_| "piano".to_string()).unwrap(),
    )
    .unwrap();
    let out: Value = serde_json::from_str(&run(
        env!("CARGO_BIN_EXE_transcription-score"),
        &[path.to_str().unwrap(), shifted.to_str().unwrap()],
    ))
    .unwrap();
    assert_eq!(out["f1"], 1.0, "{out}");
    assert_eq!(out["f1_with_offset"], 1.0, "{out}");
    assert_eq!(out["shift_ms"], -100, "{out}");
}

fn transcribed_recall(engine: &str) -> f64 {
    let dir = tempfile::tempdir().unwrap();
    let wav = dir.path().join("melody.wav");
    write_melody(&wav);
    let truth = dir.path().join("truth.mid");
    reference(&truth);
    let out_dir = dir.path().join("out");
    run(
        env!("CARGO_BIN_EXE_transcribe"),
        &[
            wav.to_str().unwrap(),
            "-o",
            out_dir.to_str().unwrap(),
            "-e",
            engine,
        ],
    );
    let estimate = out_dir.join("melody.mid");
    let score: Value = serde_json::from_str(&run(
        env!("CARGO_BIN_EXE_transcription-score"),
        &[truth.to_str().unwrap(), estimate.to_str().unwrap()],
    ))
    .unwrap();
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
