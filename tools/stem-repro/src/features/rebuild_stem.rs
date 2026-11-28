use std::path::Path;

use anyhow::Result;
use rand::SeedableRng;
use rand::rngs::StdRng;
use rand_distr::{Distribution, StandardNormal};
use rustfft::num_complex::Complex;
use serde_json::{Value, json};

use crate::shared::signal::{Format, Frame, from_mid_side, mid_side, stft, write};
use crate::shared::song::Song;

pub fn resynth(x: &[Frame], n: usize, seed: u64) -> Result<Vec<Frame>> {
    let mut rng = StdRng::seed_from_u64(seed);
    let (mid, side) = mid_side(x);
    let mut rebuilt = Vec::with_capacity(2);
    for channel in [mid, side] {
        let mut spectrum = stft(&channel, n)?;
        for frame in &mut spectrum.frames {
            for c in frame.iter_mut() {
                let noise: Complex<f64> = Complex::new(
                    StandardNormal.sample(&mut rng),
                    StandardNormal.sample(&mut rng),
                );
                *c = noise / noise.norm() * c.norm();
            }
        }
        let mut y = spectrum.inverse()?;
        y.resize(x.len(), 0.0);
        rebuilt.push(y);
    }
    let side = std::mem::take(&mut rebuilt[1]);
    Ok(from_mid_side(&rebuilt[0], &side))
}

pub fn run(song: &Song, stem: &str, fft: usize, out: &Path) -> Result<Value> {
    write(
        out,
        &resynth(&song.stem(stem, None)?, fft, 0)?,
        Format::Float,
    )?;
    Ok(json!({"stem": stem, "out": out}))
}
