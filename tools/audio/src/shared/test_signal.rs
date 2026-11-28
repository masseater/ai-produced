use std::path::Path;

use num_traits::ToPrimitive;
use serde_json::Value;

pub(crate) fn write_wav(path: &Path, rate: u32, channels: &[Vec<f64>]) {
    let spec = hound::WavSpec {
        channels: u16::try_from(channels.len()).unwrap(),
        sample_rate: rate,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut writer = hound::WavWriter::create(path, spec).unwrap();
    for i in 0..channels[0].len() {
        for channel in channels {
            writer.write_sample(channel[i].to_f32().unwrap()).unwrap();
        }
    }
    writer.finalize().unwrap();
}

pub(crate) fn near(value: &Value, expected: f64, tolerance: f64) {
    let v = value.as_f64().unwrap();
    assert!(
        (v - expected).abs() <= tolerance,
        "{v} is not within {tolerance} of {expected}"
    );
}
