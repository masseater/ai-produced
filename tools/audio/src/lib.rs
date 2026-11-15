use std::fs::File;
use std::path::Path;

use anyhow::{Context, Result, anyhow};
use ndarray::Array1;
use sonara::core::audio::resample;
use symphonia::core::codecs::audio::AudioDecoderOptions;
use symphonia::core::errors::Error;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, TrackType};
use symphonia::core::io::{MediaSourceStream, MediaSourceStreamOptions};
use symphonia::core::meta::MetadataOptions;

pub struct Decoded {
    pub samples: Vec<f32>,
    pub channels: usize,
    pub rate: u32,
}

pub fn decode(path: &Path) -> Result<Decoded> {
    let file = File::open(path).with_context(|| path.display().to_string())?;
    let stream = MediaSourceStream::new(Box::new(file), MediaSourceStreamOptions::default());
    let mut hint = Hint::new();
    if let Some(extension) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(extension);
    }
    let mut format = symphonia::default::get_probe().probe(
        &hint,
        stream,
        FormatOptions::default(),
        MetadataOptions::default(),
    )?;
    let track = format
        .default_track(TrackType::Audio)
        .ok_or_else(|| anyhow!("no audio track in {}", path.display()))?;
    let id = track.id;
    let params = track
        .codec_params
        .as_ref()
        .and_then(|p| p.audio())
        .ok_or_else(|| anyhow!("no audio parameters in {}", path.display()))?
        .clone();
    let mut decoder = symphonia::default::get_codecs()
        .make_audio_decoder(&params, &AudioDecoderOptions::default())?;
    let mut decoded = Decoded {
        samples: Vec::new(),
        channels: 0,
        rate: 0,
    };
    while let Some(packet) = format.next_packet()? {
        if packet.track_id != id {
            continue;
        }
        let buffer = match decoder.decode(&packet) {
            Ok(buffer) => buffer,
            Err(Error::DecodeError(_)) => continue,
            Err(error) => return Err(error.into()),
        };
        decoded.channels = buffer.spec().channels().count();
        decoded.rate = buffer.spec().rate();
        let start = decoded.samples.len();
        decoded
            .samples
            .resize(start + buffer.samples_interleaved(), 0.0);
        buffer.copy_to_slice_interleaved(&mut decoded.samples[start..]);
    }
    if decoded.channels == 0 {
        return Err(anyhow!("no audio decoded from {}", path.display()));
    }
    Ok(decoded)
}

impl Decoded {
    #[must_use]
    pub fn frames(&self) -> usize {
        self.samples.len() / self.channels
    }

    fn span(&self, offset: f64, duration: Option<f64>) -> (usize, usize) {
        let rate = f64::from(self.rate);
        let start = ((offset * rate) as usize).min(self.frames());
        let end = duration.map_or(self.frames(), |d| {
            (start + (d * rate) as usize).min(self.frames())
        });
        (start, end)
    }

    fn channel(&self, channel: usize, start: usize, end: usize) -> Array1<f32> {
        let channel = channel.min(self.channels - 1);
        (start..end)
            .map(|frame| self.samples[frame * self.channels + channel])
            .collect()
    }

    pub fn stereo(
        &self,
        rate: u32,
        offset: f64,
        duration: Option<f64>,
    ) -> Result<[Array1<f32>; 2]> {
        let (start, end) = self.span(offset, duration);
        Ok([
            resample(self.channel(0, start, end).view(), self.rate, rate)?,
            resample(self.channel(1, start, end).view(), self.rate, rate)?,
        ])
    }

    pub fn mono(&self, rate: u32) -> Result<Array1<f32>> {
        let mono: Array1<f32> = (0..self.frames())
            .map(|frame| {
                let at = frame * self.channels;
                self.samples[at..at + self.channels].iter().sum::<f32>() / self.channels as f32
            })
            .collect();
        Ok(resample(mono.view(), self.rate, rate)?)
    }
}

#[must_use]
pub fn round(value: f64, digits: i32) -> f64 {
    let scale = 10f64.powi(digits);
    (value * scale).round_ties_even() / scale
}

#[must_use]
pub fn percentile(values: &[f64], q: f64) -> f64 {
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let position = q / 100.0 * (sorted.len() - 1) as f64;
    let low = position.floor() as usize;
    let high = (low + 1).min(sorted.len() - 1);
    sorted[low] + (sorted[high] - sorted[low]) * (position - low as f64)
}

pub fn mean(values: impl IntoIterator<Item = f64>) -> f64 {
    let (sum, count) = values
        .into_iter()
        .fold((0.0, 0usize), |(sum, count), v| (sum + v, count + 1));
    sum / count as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percentile_interpolates_like_numpy() {
        let values = [1.0, 2.0, 3.0, 4.0, 10.0];
        assert!((percentile(&values, 90.0) - 7.6).abs() < 1e-12);
        assert!((percentile(&values, 10.0) - 1.4).abs() < 1e-12);
    }

    #[test]
    fn rounds_to_digits() {
        assert!((round(1.23456, 2) - 1.23).abs() < 1e-12);
        assert!((round(-6.66, 1) + 6.7).abs() < 1e-12);
    }
}
