use std::f64::consts::PI;
use std::fs;
use std::path::Path;
use std::process::Command;

use serde_json::Value;

fn write(path: &Path, rate: u32, channels: &[Vec<f64>]) {
    let spec = hound::WavSpec {
        channels: channels.len() as u16,
        sample_rate: rate,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut writer = hound::WavWriter::create(path, spec).unwrap();
    for i in 0..channels[0].len() {
        for channel in channels {
            writer.write_sample(channel[i] as f32).unwrap();
        }
    }
    writer.finalize().unwrap();
}

fn run(bin: &str, args: &[&str]) -> Value {
    let output = Command::new(bin).args(args).output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

fn near(value: &Value, expected: f64, tolerance: f64) {
    let v = value.as_f64().unwrap();
    assert!(
        (v - expected).abs() <= tolerance,
        "{v} is not within {tolerance} of {expected}"
    );
}

#[test]
fn mix_metrics_reads_a_reference_sine() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("sine.wav");
    let rate = 48000;
    let amplitude = 10f64.powf(-23.0 / 20.0);
    let tone: Vec<f64> = (0..rate * 10)
        .map(|i| amplitude * (2.0 * PI * 1000.0 * f64::from(i) / f64::from(rate)).sin())
        .collect();
    write(&path, rate, &[tone.clone(), tone]);
    let m = run(
        env!("CARGO_BIN_EXE_mix-metrics"),
        &[path.to_str().unwrap(), "--bpm", "120"],
    );
    near(&m["bpm"], 120.0, 0.0);
    near(&m["lufs_integrated"], -23.0, 0.1);
    near(&m["true_peak_dbtp"], -23.0, 0.1);
    near(&m["stereo_side_to_mid_db"], -120.0, 0.0);
    near(&m["crest_db"], 3.01, 0.05);
    near(&m["rms_dbfs"], -26.01, 0.05);
    near(&m["mid"], 0.0, 0.05);
    near(&m["spectral_centroid_hz"], 1000.0, 30.0);
    near(&m["loudness_p90_minus_p10_db"], 0.0, 0.1);
}

fn drums(rate: u32, seconds: u32, slow_bpm: f64, snare_offset: f64) -> Vec<f64> {
    let beat = 60.0 / slow_bpm;
    let mut seed = 12345u64;
    let mut out = vec![0.0; (rate * seconds) as usize];
    for (i, sample) in out.iter_mut().enumerate() {
        let t = i as f64 / f64::from(rate);
        let since_kick = t % beat;
        let since_snare = (t - snare_offset * beat).rem_euclid(beat);
        seed = seed
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        let noise = (seed >> 33) as f64 / f64::from(1u32 << 31) - 1.0;
        let kick = (2.0 * PI * 60.0 * since_kick).sin() * (-since_kick / 0.12).exp();
        let snare = noise * (-since_snare / 0.08).exp();
        *sample = 0.5 * kick + 0.3 * snare;
    }
    out
}

#[test]
fn tempo_octave_reads_the_backbeat() {
    let dir = tempfile::tempdir().unwrap();
    let rate = 22050;
    for (offset, verdict) in [(0.5, "fast"), (0.0, "slow")] {
        let path = dir.path().join(format!("drums-{verdict}.wav"));
        write(&path, rate, &[drums(rate, 30, 80.0, offset)]);
        let result = run(
            env!("CARGO_BIN_EXE_tempo-octave"),
            &[path.to_str().unwrap(), "80", "--section", "5", "25"],
        );
        assert_eq!(result["whole"]["verdict"], verdict, "{result}");
        assert_eq!(result["section"]["verdict"], verdict, "{result}");
        near(&result["whole"]["grid_bpm"], 80.0, 1.0);
    }
}

#[test]
fn vocal_sections_skip_silence_and_track_a_steady_pitch() {
    let dir = tempfile::tempdir().unwrap();
    let rate = 22050;
    let path = dir.path().join("voice.wav");
    let bar = 2.0;
    let voice: Vec<f64> = (0..(f64::from(rate) * bar * 4.0) as usize)
        .map(|i| {
            let t = i as f64 / f64::from(rate);
            if t < bar * 2.0 {
                0.0
            } else {
                0.3 * (2.0 * PI * 440.0 * t).sin()
            }
        })
        .collect();
    write(&path, rate, &[voice]);
    let score = dir.path().join("score.toml");
    fs::write(
        &score,
        "bpm = 120\n[[sections]]\nname = \"Rest\"\nbars = 2\n[[sections]]\nname = \"Sing\"\nbars = 2\n",
    )
    .unwrap();
    let result = run(
        env!("CARGO_BIN_EXE_vocal-sections"),
        &[path.to_str().unwrap(), "--score", score.to_str().unwrap()],
    );
    assert!(result["Rest"].is_null(), "{result}");
    let sing = &result["Sing"];
    near(&sing["sound_ratio"], 1.0, 0.02);
    near(&sing["voiced_of_sound"], 1.0, 0.05);
    near(&sing["centroid_hz"], 440.0, 40.0);
    near(&sing["pitch_wobble_cents"], 0.0, 2.0);
    near(&sing["rms_db"], -13.5, 0.2);
}
