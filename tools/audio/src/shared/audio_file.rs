use std::fs::File;
use std::path::Path;

use anyhow::{Context, Result, anyhow};
use ndarray::Array1;
use num_traits::ToPrimitive;
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
    let mut codec = symphonia::default::get_codecs()
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
        let buffer = match codec.decode(&packet) {
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

    fn span(&self, offset: f64, duration: Option<f64>) -> Result<(usize, usize)> {
        let rate = f64::from(self.rate);
        let start = (offset * rate)
            .to_usize()
            .with_context(|| format!("bad offset {offset}"))?
            .min(self.frames());
        let end = match duration {
            Some(d) => (start
                + (d * rate)
                    .to_usize()
                    .with_context(|| format!("bad duration {d}"))?)
            .min(self.frames()),
            None => self.frames(),
        };
        Ok((start, end))
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
        let (start, end) = self.span(offset, duration)?;
        Ok([
            resample(self.channel(0, start, end).view(), self.rate, rate)?,
            resample(self.channel(1, start, end).view(), self.rate, rate)?,
        ])
    }

    pub fn mono(&self, rate: u32) -> Result<Array1<f32>> {
        let channels = self.channels.to_f32().context("channel count")?;
        let mono: Array1<f32> = (0..self.frames())
            .map(|frame| {
                let at = frame * self.channels;
                self.samples[at..at + self.channels].iter().sum::<f32>() / channels
            })
            .collect();
        Ok(resample(mono.view(), self.rate, rate)?)
    }
}
