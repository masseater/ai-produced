use std::fs;
use std::path::Path;

use num_traits::ToPrimitive;

use crate::shared::midi::{self, Note};

pub(crate) const MELODY: [(u8, f64, f64); 7] = [
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

pub(crate) fn write_melody(path: &Path) {
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

pub(crate) fn write_reference(path: &Path, shift: f64) {
    let notes: Vec<Note> = MELODY
        .iter()
        .map(|(pitch, onset, offset)| Note {
            program: 0,
            pitch: *pitch,
            onset: onset + shift,
            offset: offset + shift,
        })
        .collect();
    fs::write(path, midi::write(&notes, |_| "piano".to_string()).unwrap()).unwrap();
}
